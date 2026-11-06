use anyhow::{Result, ensure};
use hickory_proto::{
    op::{Message, MessageType, Query, ResponseCode},
    rr::{Name, RData, RecordType},
};
use hickory_resolver::TokioResolver;
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
    task::JoinSet,
    time::timeout,
};

pub(super) fn challenge_names(domains: &[String]) -> BTreeMap<String, Vec<String>> {
    let mut names = BTreeMap::<_, Vec<_>>::new();
    for domain in domains {
        let base = domain
            .trim_end_matches('.')
            .trim_start_matches("*.")
            .to_ascii_lowercase();
        names
            .entry(format!("_acme-challenge.{base}"))
            .or_default()
            .push(domain.clone());
    }
    names
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(super) struct Observation {
    pub zone: String,
    pub values: BTreeSet<String>,
    pub delegation: BTreeSet<String>,
}

/// Ask each parent-zone authority directly. A referral at the challenge name is a
/// delegation even when the recursive resolver would simply follow it for TXT.
#[cfg(not(test))]
pub(super) async fn observe(name: String) -> Result<Observation> {
    timeout(Duration::from_secs(12), observe_inner(&name)).await?
}
#[cfg(test)]
pub(super) fn observe(
    name: String,
) -> impl std::future::Future<Output = Result<Observation>> + Send + 'static {
    // Capture the test resolver on the caller's task before fanning out over SANs.
    #[cfg(test)]
    let fixture = TEST_DNS
        .try_with(|records| records.get(&name).cloned())
        .ok();
    async move {
        #[cfg(test)]
        if let Some(fixture) = fixture {
            return fixture.ok_or_else(|| anyhow::anyhow!("mock DNS failure"));
        }
        if name.ends_with(".test"){anyhow::bail!("no DNS fixture for {name}");}
        timeout(Duration::from_secs(12), observe_inner(&name)).await?
    }
}
#[cfg(test)]
tokio::task_local! {pub(super) static TEST_DNS: BTreeMap<String,Observation>;}

async fn observe_inner(name: &str) -> Result<Observation> {
    let resolver = TokioResolver::builder_tokio()?.build()?;
    let mut zone = name
        .split_once('.')
        .ok_or_else(|| anyhow::anyhow!("invalid name"))?
        .1;
    let servers = loop {
        match resolver.ns_lookup(format!("{zone}.")).await {
            Ok(lookup) => {
                let servers: BTreeSet<_> = lookup
                    .answers()
                    .iter()
                    .filter(|r| {
                        r.name
                            .to_ascii()
                            .trim_end_matches('.')
                            .eq_ignore_ascii_case(zone)
                    })
                    .filter_map(|r| match &r.data {
                        RData::NS(ns) => Some(ns.0.to_ascii()),
                        _ => None,
                    })
                    .collect();
                if !servers.is_empty() {
                    break servers;
                }
            }
            Err(e) if e.is_no_records_found() && e.clone().into_soa().is_some() => {}
            Err(e) => return Err(e.into()),
        }
        zone = zone
            .split_once('.')
            .ok_or_else(|| anyhow::anyhow!("zone unavailable"))?
            .1;
    };
    ensure!(zone.contains('.'), "domain has no delegated zone");
    observe_authorities(resolver, name, zone, servers, true).await
}

pub(super) async fn delegated_txt(name: &str, servers: BTreeSet<String>) -> Result<Observation> {
    timeout(
        Duration::from_secs(12),
        observe_authorities(
            TokioResolver::builder_tokio()?.build()?,
            name,
            name,
            servers,
            false,
        ),
    )
    .await?
}

async fn observe_authorities(
    resolver: TokioResolver,
    name: &str,
    zone: &str,
    servers: BTreeSet<String>,
    check_ns: bool,
) -> Result<Observation> {
    ensure!(servers.len() <= 32, "too many authorities");
    let mut tasks = JoinSet::new();
    for server in servers {
        let resolver = resolver.clone();
        let name = name.to_owned();
        tasks.spawn(async move {
            let addresses = resolver.lookup_ip(server).await?;
            for ip in addresses.iter().filter(|ip| match ip {
                IpAddr::V4(v) => crate::ddns::is_global_ipv4(*v),
                IpAddr::V6(v) => crate::ddns::is_global_ipv6(*v),
            }) {
                if let Ok(Ok(observation)) =
                    timeout(Duration::from_secs(3), query_authority(ip, &name, check_ns)).await
                {
                    return Ok(observation);
                }
            }
            anyhow::bail!("authority unavailable")
        });
    }
    let mut agreed = None;
    while let Some(result) = tasks.join_next().await {
        let observation = result??;
        ensure!(
            agreed
                .as_ref()
                .is_none_or(|previous| previous == &observation),
            "inconsistent authorities"
        );
        agreed = Some(observation);
    }
    let mut agreed = agreed.ok_or_else(|| anyhow::anyhow!("no authority"))?;
    agreed.zone = zone.to_string();
    Ok(agreed)
}

async fn query_authority(ip: IpAddr, owner: &str, check_ns: bool) -> Result<Observation> {
    if !check_ns {
        return query_record(ip, owner, RecordType::TXT).await;
    }
    let (mut txt, ns) = tokio::try_join!(
        query_record(ip, owner, RecordType::TXT),
        query_record(ip, owner, RecordType::NS)
    )?;
    txt.delegation.extend(ns.delegation);
    Ok(txt)
}

async fn query_record(ip: IpAddr, owner: &str, rrtype: RecordType) -> Result<Observation> {
    let name = Name::from_ascii(format!("{owner}."))?;
    let mut request = Message::query();
    request.queries.push(Query::query(name.clone(), rrtype));
    let bytes = request.to_vec()?;
    let socket = UdpSocket::bind(if ip.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).await?;
    socket.connect((ip, 53)).await?;
    socket.send(&bytes).await?;
    let mut udp = [0; 4096];
    let received = timeout(Duration::from_millis(1200), socket.recv(&mut udp)).await;
    let datagram = received
        .ok()
        .and_then(Result::ok)
        .and_then(|n| Message::from_vec(&udp[..n]).ok());
    let reply = match datagram {
        Some(reply) if !reply.metadata.truncation => reply,
        _ => {
            let mut stream = TcpStream::connect((ip, 53)).await?;
            stream.write_u16(u16::try_from(bytes.len())?).await?;
            stream.write_all(&bytes).await?;
            let length = stream.read_u16().await?;
            let mut buffer = vec![0; usize::from(length)];
            stream.read_exact(&mut buffer).await?;
            Message::from_vec(&buffer)?
        }
    };
    ensure!(
        reply.metadata.id == request.metadata.id
            && reply.queries == request.queries
            && reply.metadata.message_type == MessageType::Response
            && !reply.metadata.truncation,
        "invalid DNS reply"
    );
    interpret(&name, &reply)
}

fn interpret(name: &Name, reply: &Message) -> Result<Observation> {
    ensure!(
        matches!(
            reply.metadata.response_code,
            ResponseCode::NoError | ResponseCode::NXDomain
        ),
        "DNS lookup failed"
    );
    let mut result = Observation::default();
    for record in reply
        .answers
        .iter()
        .chain(&reply.authorities)
        .filter(|r| &r.name == name)
    {
        match &record.data {
            RData::CNAME(alias) => {
                result.delegation.insert(format!("CNAME {}", alias.0));
            }
            RData::NS(ns) => {
                result.delegation.insert(format!("NS {}", ns.0));
            }
            RData::TXT(txt) => {
                let bytes = txt
                    .txt_data
                    .iter()
                    .flat_map(|s| s.iter().copied())
                    .collect::<Vec<_>>();
                result.values.insert(String::from_utf8(bytes)?);
            }
            _ => {}
        }
    }
    ensure!(
        reply.metadata.authoritative || !result.delegation.is_empty(),
        "non-authoritative answer"
    );
    ensure!(
        !result.values.is_empty()
            || !result.delegation.is_empty()
            || reply
                .authorities
                .iter()
                .any(|r| matches!(r.data, RData::SOA(_)) && r.name.zone_of(name)),
        "missing negative proof"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::{
        op::OpCode,
        rr::{
            Record,
            rdata::{CNAME, NS, TXT},
        },
    };
    #[test]
    fn responses_require_authority_and_preserve_txt_chunks_and_delegations() {
        let name = Name::from_ascii("_acme-challenge.example.test.").unwrap();
        let mut msg = Message::response(1, OpCode::Query);
        msg.answers.push(Record::from_rdata(
            name.clone(),
            60,
            RData::TXT(TXT::new(vec!["first".into(), "second".into()])),
        ));
        assert!(interpret(&name, &msg).is_err());
        msg.metadata.authoritative = true;
        assert!(
            interpret(&name, &msg)
                .unwrap()
                .values
                .contains("firstsecond")
        );
        msg.answers.clear();
        msg.metadata.response_code = ResponseCode::ServFail;
        assert!(interpret(&name, &msg).is_err());
        msg.metadata.response_code = ResponseCode::NoError;
        assert!(interpret(&name, &msg).is_err());
        let target = Name::from_ascii("verify.other.test.").unwrap();
        msg.authorities.push(Record::from_rdata(
            name.clone(),
            60,
            RData::NS(NS(target.clone())),
        ));
        msg.metadata.authoritative = false;
        assert!(
            interpret(&name, &msg)
                .unwrap()
                .delegation
                .contains("NS verify.other.test.")
        );
        msg.authorities.clear();
        msg.metadata.authoritative = true;
        msg.answers.push(Record::from_rdata(
            name.clone(),
            60,
            RData::CNAME(CNAME(target)),
        ));
        assert!(
            interpret(&name, &msg)
                .unwrap()
                .delegation
                .contains("CNAME verify.other.test.")
        );
    }
}
