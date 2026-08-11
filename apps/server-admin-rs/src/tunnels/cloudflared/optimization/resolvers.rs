use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use hickory_proto::{
    op::{Message, MessageType, OpCode, Query, ResponseCode},
    rr::{DNSClass, Name, RData, RecordType},
};
use ipnet::Ipv4Net;
use serde::Serialize;

const DOH_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_DNS_MESSAGE_BYTES: usize = u16::MAX as usize;
const DNS_MESSAGE_MEDIA_TYPE: &str = "application/dns-message";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DohProvider {
    id: &'static str,
    hostname: &'static str,
    endpoint: &'static str,
    bootstrap_ipv4: &'static [[u8; 4]],
}

const DOH_PROVIDERS: &[DohProvider] = &[
    DohProvider {
        id: "cloudflare",
        hostname: "cloudflare-dns.com",
        endpoint: "https://cloudflare-dns.com/dns-query",
        bootstrap_ipv4: &[[1, 1, 1, 1], [1, 0, 0, 1]],
    },
    DohProvider {
        id: "google",
        hostname: "dns.google",
        endpoint: "https://dns.google/dns-query",
        bootstrap_ipv4: &[[8, 8, 8, 8], [8, 8, 4, 4]],
    },
    DohProvider {
        id: "dnspod",
        hostname: "doh.pub",
        endpoint: "https://doh.pub/dns-query",
        bootstrap_ipv4: &[[1, 12, 12, 12], [120, 53, 53, 53]],
    },
    DohProvider {
        id: "alidns",
        hostname: "dns.alidns.com",
        endpoint: "https://dns.alidns.com/dns-query",
        bootstrap_ipv4: &[[223, 5, 5, 5], [223, 6, 6, 6]],
    },
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResolverDiagnostic {
    pub(super) provider: String,
    pub(super) status: String,
    pub(super) success_count: usize,
    pub(super) failure_count: usize,
    pub(super) last_error_code: Option<String>,
    pub(super) last_error_detail: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ResolverAttempt {
    provider: &'static str,
    verified_ips: Vec<Ipv4Addr>,
    failure: Option<ResolverFailure>,
}

impl ResolverAttempt {
    #[cfg(test)]
    fn success(provider: &'static str, verified_ips: Vec<Ipv4Addr>) -> Self {
        Self {
            provider,
            verified_ips,
            failure: None,
        }
    }

    #[cfg(test)]
    fn failed(provider: &'static str, code: &'static str) -> Self {
        Self {
            provider,
            verified_ips: Vec::new(),
            failure: Some(ResolverFailure::new(code, "test failure")),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ResolverFailure {
    code: &'static str,
    detail: String,
}

impl ResolverFailure {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Debug)]
pub(super) struct CandidateResolution {
    pub(super) ips: Vec<Ipv4Addr>,
    pub(super) attempts: Vec<ResolverAttempt>,
}

impl CandidateResolution {
    pub(super) fn all_failed_summary(&self) -> Option<String> {
        self.attempts
            .iter()
            .all(|attempt| attempt.failure.is_some())
            .then(|| {
                let details = self
                    .attempts
                    .iter()
                    .filter_map(|attempt| {
                        attempt
                            .failure
                            .as_ref()
                            .map(|failure| format!("{}={}", attempt.provider, failure.code))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("all DoH resolvers failed ({details})")
            })
    }

    pub(super) fn failed_for_all_providers(failure: ResolverFailure) -> Self {
        Self {
            ips: Vec::new(),
            attempts: DOH_PROVIDERS
                .iter()
                .map(|provider| ResolverAttempt {
                    provider: provider.id,
                    verified_ips: Vec::new(),
                    failure: Some(failure.clone()),
                })
                .collect(),
        }
    }
}

pub(super) fn build_doh_client() -> Result<reqwest::Client, ResolverFailure> {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(DOH_TIMEOUT)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none());
    for provider in DOH_PROVIDERS {
        let addresses = provider
            .bootstrap_ipv4
            .iter()
            .map(|octets| SocketAddr::new(IpAddr::V4(Ipv4Addr::from(*octets)), 443))
            .collect::<Vec<_>>();
        builder = builder.resolve_to_addrs(provider.hostname, &addresses);
    }
    builder.build().map_err(|_| {
        ResolverFailure::new(
            "client-initialization-error",
            "secure resolver client could not be initialized",
        )
    })
}

pub(super) async fn resolve_candidate_hostname(
    client: &reqwest::Client,
    hostname: &str,
    prefixes: &[Ipv4Net],
) -> CandidateResolution {
    let (cloudflare, google, dnspod, alidns) = tokio::join!(
        resolve_with_provider(client, DOH_PROVIDERS[0], hostname, prefixes),
        resolve_with_provider(client, DOH_PROVIDERS[1], hostname, prefixes),
        resolve_with_provider(client, DOH_PROVIDERS[2], hostname, prefixes),
        resolve_with_provider(client, DOH_PROVIDERS[3], hostname, prefixes),
    );
    finalize_resolution(vec![cloudflare, google, dnspod, alidns])
}

async fn resolve_with_provider(
    client: &reqwest::Client,
    provider: DohProvider,
    hostname: &str,
    prefixes: &[Ipv4Net],
) -> ResolverAttempt {
    match query_doh_ipv4(client, provider, hostname).await {
        Ok(answers) => ResolverAttempt {
            provider: provider.id,
            verified_ips: verified_cloudflare_ips(answers, prefixes),
            failure: None,
        },
        Err(failure) => ResolverAttempt {
            provider: provider.id,
            verified_ips: Vec::new(),
            failure: Some(failure),
        },
    }
}

fn verified_cloudflare_ips(answers: HashSet<Ipv4Addr>, prefixes: &[Ipv4Net]) -> Vec<Ipv4Addr> {
    let mut verified = answers
        .into_iter()
        .filter(|ip| prefixes.iter().any(|prefix| prefix.contains(ip)))
        .collect::<Vec<_>>();
    verified.sort();
    verified.dedup();
    verified
}

fn finalize_resolution(attempts: Vec<ResolverAttempt>) -> CandidateResolution {
    let mut votes = HashMap::<Ipv4Addr, usize>::new();
    for attempt in &attempts {
        let mut provider_ips = HashSet::new();
        for ip in &attempt.verified_ips {
            if provider_ips.insert(*ip) {
                *votes.entry(*ip).or_default() += 1;
            }
        }
    }
    let mut ips = votes.into_iter().collect::<Vec<_>>();
    ips.sort_by(|(left_ip, left_votes), (right_ip, right_votes)| {
        right_votes
            .cmp(left_votes)
            .then_with(|| left_ip.cmp(right_ip))
    });
    CandidateResolution {
        ips: ips.into_iter().map(|(ip, _)| ip).collect(),
        attempts,
    }
}

pub(super) fn aggregate_resolver_diagnostics(
    attempts: &[ResolverAttempt],
) -> Vec<ResolverDiagnostic> {
    DOH_PROVIDERS
        .iter()
        .filter_map(|provider| {
            let provider_attempts = attempts
                .iter()
                .filter(|attempt| attempt.provider == provider.id)
                .collect::<Vec<_>>();
            if provider_attempts.is_empty() {
                return None;
            }
            let success_count = provider_attempts
                .iter()
                .filter(|attempt| attempt.failure.is_none())
                .count();
            let failure_count = provider_attempts.len().saturating_sub(success_count);
            let last_failure = provider_attempts
                .iter()
                .rev()
                .find_map(|attempt| attempt.failure.as_ref());
            let status = if failure_count == 0 {
                "healthy"
            } else if success_count > 0 {
                "degraded"
            } else {
                "unavailable"
            };
            Some(ResolverDiagnostic {
                provider: provider.id.to_string(),
                status: status.to_string(),
                success_count,
                failure_count,
                last_error_code: last_failure.map(|failure| failure.code.to_string()),
                last_error_detail: last_failure.map(|failure| failure.detail.clone()),
            })
        })
        .collect()
}

pub(super) fn initial_resolution_path(
    doh_candidates_available: bool,
    official_ranges: bool,
) -> &'static str {
    if doh_candidates_available {
        "multi-doh"
    } else if official_ranges {
        "official-ranges"
    } else {
        "unavailable"
    }
}

async fn query_doh_ipv4(
    client: &reqwest::Client,
    provider: DohProvider,
    hostname: &str,
) -> Result<HashSet<Ipv4Addr>, ResolverFailure> {
    query_doh_ipv4_with_timeout(client, provider, hostname, DOH_TIMEOUT).await
}

async fn query_doh_ipv4_with_timeout(
    client: &reqwest::Client,
    provider: DohProvider,
    hostname: &str,
    timeout: Duration,
) -> Result<HashSet<Ipv4Addr>, ResolverFailure> {
    let query = build_dns_query(hostname)?;
    let response = client
        .post(provider.endpoint)
        .header(reqwest::header::ACCEPT, DNS_MESSAGE_MEDIA_TYPE)
        .header(reqwest::header::CONTENT_TYPE, DNS_MESSAGE_MEDIA_TYPE)
        .timeout(timeout)
        .body(query)
        .send()
        .await
        .map_err(classify_request_error)?;
    if !response.status().is_success() {
        return Err(ResolverFailure::new(
            "http-error",
            format!("resolver returned HTTP {}", response.status().as_u16()),
        ));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if !content_type.is_some_and(|value| value.eq_ignore_ascii_case(DNS_MESSAGE_MEDIA_TYPE)) {
        return Err(ResolverFailure::new(
            "invalid-content-type",
            "resolver did not return application/dns-message",
        ));
    }

    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify_request_error)? {
        if body.len().saturating_add(chunk.len()) > MAX_DNS_MESSAGE_BYTES {
            return Err(ResolverFailure::new(
                "response-too-large",
                "resolver response exceeded 65535 bytes",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    parse_dns_response(hostname, &body)
}

fn build_dns_query(hostname: &str) -> Result<Vec<u8>, ResolverFailure> {
    let normalized = format!("{}.", hostname.trim_end_matches('.'));
    let name = Name::from_ascii(&normalized).map_err(|_| {
        ResolverFailure::new(
            "invalid-hostname",
            "candidate hostname is not valid DNS syntax",
        )
    })?;
    let mut message = Message::new(0, MessageType::Query, OpCode::Query);
    message.metadata.recursion_desired = true;
    message.add_query(Query::query(name, RecordType::A));
    message
        .to_vec()
        .map_err(|_| ResolverFailure::new("query-encode-error", "DNS query could not be encoded"))
}

fn parse_dns_response(hostname: &str, body: &[u8]) -> Result<HashSet<Ipv4Addr>, ResolverFailure> {
    let normalized = format!("{}.", hostname.trim_end_matches('.'));
    let expected_name = Name::from_ascii(&normalized).map_err(|_| {
        ResolverFailure::new(
            "invalid-hostname",
            "candidate hostname is not valid DNS syntax",
        )
    })?;
    let message = Message::from_vec(body).map_err(|_| {
        ResolverFailure::new(
            "invalid-dns-response",
            "resolver returned a malformed DNS message",
        )
    })?;
    if message.metadata.id != 0
        || message.metadata.message_type != MessageType::Response
        || message.metadata.op_code != OpCode::Query
    {
        return Err(ResolverFailure::new(
            "response-mismatch",
            "resolver response header did not match the query",
        ));
    }
    if message.metadata.truncation {
        return Err(ResolverFailure::new(
            "truncated-response",
            "resolver returned a truncated DNS message",
        ));
    }
    if message.metadata.response_code != ResponseCode::NoError {
        return Err(ResolverFailure::new(
            "dns-error",
            format!(
                "resolver returned DNS status {}",
                message.metadata.response_code
            ),
        ));
    }
    if message.queries.len() != 1
        || message.queries[0].name() != &expected_name
        || message.queries[0].query_type() != RecordType::A
        || message.queries[0].query_class() != DNSClass::IN
    {
        return Err(ResolverFailure::new(
            "response-mismatch",
            "resolver response question did not match the query",
        ));
    }
    let mut current_name = expected_name;
    let mut visited_names = HashSet::from([current_name.clone()]);
    for _ in 0..=message.answers.len() {
        let cname_targets = message
            .answers
            .iter()
            .filter(|record| record.name == current_name)
            .filter_map(|record| match &record.data {
                RData::CNAME(target) => Some(target.0.clone()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        if cname_targets.len() > 1 {
            return Err(ResolverFailure::new(
                "response-mismatch",
                "resolver returned an ambiguous CNAME chain",
            ));
        }
        let Some(target) = cname_targets.into_iter().next() else {
            break;
        };
        if message
            .answers
            .iter()
            .any(|record| record.name == current_name && matches!(&record.data, RData::A(_)))
        {
            return Err(ResolverFailure::new(
                "response-mismatch",
                "resolver returned conflicting CNAME and address records",
            ));
        }
        if !visited_names.insert(target.clone()) {
            return Err(ResolverFailure::new(
                "response-mismatch",
                "resolver returned a cyclic CNAME chain",
            ));
        }
        current_name = target;
    }
    Ok(message
        .answers
        .iter()
        .filter(|record| record.name == current_name)
        .filter_map(|record| match &record.data {
            RData::A(address) => Some(Ipv4Addr::from(*address)),
            _ => None,
        })
        .collect())
}

fn classify_request_error(error: reqwest::Error) -> ResolverFailure {
    if error.is_timeout() {
        ResolverFailure::new("timeout", "resolver request timed out after 5 seconds")
    } else if error.is_connect() {
        ResolverFailure::new(
            "connect-error",
            "resolver HTTPS connection could not be established",
        )
    } else {
        ResolverFailure::new("transport-error", "resolver HTTPS request failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::{
        Record,
        rdata::{A, CNAME},
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn dns_response(hostname: &str, addresses: &[Ipv4Addr]) -> Vec<u8> {
        let name = Name::from_ascii(hostname).expect("valid test hostname");
        let mut message = Message::new(0, MessageType::Response, OpCode::Query);
        message.metadata.recursion_desired = true;
        message.metadata.recursion_available = true;
        message.add_query(Query::query(name.clone(), RecordType::A));
        for address in addresses {
            message.add_answer(Record::from_rdata(
                name.clone(),
                60,
                RData::A(A::from(*address)),
            ));
        }
        message.to_vec().expect("encode test response")
    }

    async fn bind_http_server() -> (tokio::net::TcpListener, String) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test HTTP server");
        let address = listener.local_addr().expect("test server address");
        (listener, format!("http://{address}/dns-query"))
    }

    async fn serve_http_once(
        listener: tokio::net::TcpListener,
        status: u16,
        content_type: &str,
        body: Vec<u8>,
        delay: Duration,
    ) {
        let content_type = content_type.to_string();
        let (mut stream, _) = listener.accept().await.expect("accept test request");
        let mut request = vec![0_u8; 4096];
        let _ = stream.read(&mut request).await;
        tokio::time::sleep(delay).await;
        let reason = if status == 200 { "OK" } else { "Test Error" };
        let headers = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        if stream.write_all(headers.as_bytes()).await.is_ok() {
            let _ = stream.write_all(&body).await;
        }
    }

    fn test_provider(endpoint: String) -> DohProvider {
        DohProvider {
            id: "test",
            hostname: "test.invalid",
            endpoint: Box::leak(endpoint.into_boxed_str()),
            bootstrap_ipv4: &[],
        }
    }

    #[test]
    fn rfc8484_query_and_response_round_trip() {
        let hostname = "www.icann.org";
        let query = Message::from_vec(&build_dns_query(hostname).expect("build query"))
            .expect("decode query");
        assert_eq!(query.metadata.id, 0);
        assert_eq!(query.metadata.message_type, MessageType::Query);
        assert!(query.metadata.recursion_desired);
        assert_eq!(query.queries[0].name().to_ascii(), "www.icann.org.");

        let expected = "104.18.27.68".parse().expect("valid IPv4");
        let parsed = parse_dns_response(hostname, &dns_response(hostname, &[expected]))
            .expect("parse response");
        assert_eq!(parsed, HashSet::from([expected]));
    }

    #[test]
    fn rejects_malformed_and_mismatched_dns_responses() {
        assert_eq!(
            parse_dns_response("www.icann.org", b"not dns")
                .expect_err("malformed response must fail")
                .code,
            "invalid-dns-response"
        );
        let response = dns_response("other.example", &[]);
        assert_eq!(
            parse_dns_response("www.icann.org", &response)
                .expect_err("mismatched question must fail")
                .code,
            "response-mismatch"
        );

        let name = Name::from_ascii("www.icann.org").expect("valid hostname");
        let mut query = Query::query(name, RecordType::A);
        query.set_query_class(DNSClass::CH);
        let mut wrong_class = Message::new(0, MessageType::Response, OpCode::Query);
        wrong_class.add_query(query);
        assert_eq!(
            parse_dns_response(
                "www.icann.org",
                &wrong_class.to_vec().expect("encode wrong class response"),
            )
            .expect_err("non-IN response question must fail")
            .code,
            "response-mismatch"
        );
    }

    #[test]
    fn rejects_dns_errors_and_unrelated_forged_answers() {
        let hostname = "www.icann.org";
        let name = Name::from_ascii(hostname).expect("valid hostname");
        let mut failed = Message::new(0, MessageType::Response, OpCode::Query);
        failed.metadata.response_code = ResponseCode::ServFail;
        failed.add_query(Query::query(name.clone(), RecordType::A));
        assert_eq!(
            parse_dns_response(hostname, &failed.to_vec().expect("encode DNS error"))
                .expect_err("DNS error must fail")
                .code,
            "dns-error"
        );

        let forged: Ipv4Addr = "104.18.26.94".parse().expect("valid IPv4");
        let mut response = Message::new(0, MessageType::Response, OpCode::Query);
        response.add_query(Query::query(name, RecordType::A));
        response.add_answer(Record::from_rdata(
            Name::from_ascii("unrelated.example").expect("valid answer name"),
            60,
            RData::A(A::from(forged)),
        ));
        assert!(
            parse_dns_response(
                hostname,
                &response.to_vec().expect("encode forged response")
            )
            .expect("valid response with irrelevant answer")
            .is_empty()
        );
    }

    #[test]
    fn follows_only_the_queried_names_cname_chain() {
        let hostname = "www.icann.org";
        let query_name = Name::from_ascii(hostname).expect("valid hostname");
        let canonical_name = Name::from_ascii("edge.icann.org").expect("valid canonical name");
        let expected = "104.18.26.94".parse().expect("valid IPv4");
        let mut response = Message::new(0, MessageType::Response, OpCode::Query);
        response.add_query(Query::query(query_name.clone(), RecordType::A));
        response.add_answer(Record::from_rdata(
            query_name,
            60,
            RData::CNAME(CNAME(canonical_name.clone())),
        ));
        response.add_answer(Record::from_rdata(
            canonical_name,
            60,
            RData::A(A::from(expected)),
        ));
        assert_eq!(
            parse_dns_response(hostname, &response.to_vec().expect("encode CNAME response"))
                .expect("parse CNAME response"),
            HashSet::from([expected])
        );
    }

    #[test]
    fn rejects_ambiguous_or_cyclic_cname_chains() {
        let hostname = "www.icann.org";
        let query_name = Name::from_ascii(hostname).expect("valid hostname");
        let first_target = Name::from_ascii("edge-a.icann.org").expect("valid target");
        let second_target = Name::from_ascii("edge-b.icann.org").expect("valid target");
        let mut ambiguous = Message::new(0, MessageType::Response, OpCode::Query);
        ambiguous.add_query(Query::query(query_name.clone(), RecordType::A));
        for target in [first_target, second_target] {
            ambiguous.add_answer(Record::from_rdata(
                query_name.clone(),
                60,
                RData::CNAME(CNAME(target)),
            ));
        }
        assert_eq!(
            parse_dns_response(
                hostname,
                &ambiguous.to_vec().expect("encode ambiguous response"),
            )
            .expect_err("ambiguous CNAME response must fail")
            .code,
            "response-mismatch"
        );

        let loop_target = Name::from_ascii("loop.icann.org").expect("valid target");
        let mut cyclic = Message::new(0, MessageType::Response, OpCode::Query);
        cyclic.add_query(Query::query(query_name.clone(), RecordType::A));
        cyclic.add_answer(Record::from_rdata(
            query_name.clone(),
            60,
            RData::CNAME(CNAME(loop_target.clone())),
        ));
        cyclic.add_answer(Record::from_rdata(
            loop_target,
            60,
            RData::CNAME(CNAME(query_name)),
        ));
        assert_eq!(
            parse_dns_response(hostname, &cyclic.to_vec().expect("encode cyclic response"))
                .expect_err("cyclic CNAME response must fail")
                .code,
            "response-mismatch"
        );
    }

    #[test]
    fn dedicated_doh_client_uses_all_bootstrap_addresses() {
        assert!(build_doh_client().is_ok());
        assert!(
            DOH_PROVIDERS
                .iter()
                .all(|provider| !provider.bootstrap_ipv4.is_empty())
        );
    }

    #[tokio::test]
    async fn classifies_http_errors_and_timeouts() {
        let client = reqwest::Client::new();
        let (listener, endpoint) = bind_http_server().await;
        let request = query_doh_ipv4_with_timeout(
            &client,
            test_provider(endpoint),
            "www.icann.org",
            Duration::from_secs(1),
        );
        let (_, result) = tokio::join!(
            serve_http_once(
                listener,
                503,
                DNS_MESSAGE_MEDIA_TYPE,
                Vec::new(),
                Duration::ZERO,
            ),
            request,
        );
        assert_eq!(
            result.expect_err("HTTP failure must fail").code,
            "http-error"
        );

        let (listener, endpoint) = bind_http_server().await;
        let request = query_doh_ipv4_with_timeout(
            &client,
            test_provider(endpoint),
            "www.icann.org",
            Duration::from_millis(10),
        );
        let (_, result) = tokio::join!(
            serve_http_once(
                listener,
                200,
                DNS_MESSAGE_MEDIA_TYPE,
                dns_response("www.icann.org", &[]),
                Duration::from_millis(100),
            ),
            request,
        );
        assert_eq!(
            result.expect_err("slow resolver must time out").code,
            "timeout"
        );
    }

    #[test]
    fn consensus_addresses_rank_before_single_resolver_fallbacks() {
        let consensus = "104.18.26.94".parse().expect("valid IPv4");
        let fallback = "172.64.10.20".parse().expect("valid IPv4");
        let resolution = finalize_resolution(vec![
            ResolverAttempt::success("cloudflare", vec![consensus]),
            ResolverAttempt::success("google", vec![consensus, fallback]),
            ResolverAttempt::failed("dnspod", "timeout"),
            ResolverAttempt::failed("alidns", "connect-error"),
        ]);
        assert_eq!(resolution.ips, vec![consensus, fallback]);
    }

    #[test]
    fn mainland_and_global_resolvers_can_fail_independently() {
        let domestic = "104.18.26.94".parse().expect("valid IPv4");
        let domestic_fallback = finalize_resolution(vec![
            ResolverAttempt::failed("cloudflare", "timeout"),
            ResolverAttempt::failed("google", "connect-error"),
            ResolverAttempt::success("dnspod", vec![domestic]),
            ResolverAttempt::success("alidns", vec![domestic]),
        ]);
        assert_eq!(domestic_fallback.ips, vec![domestic]);

        let global = "172.64.10.20".parse().expect("valid IPv4");
        let global_fallback = finalize_resolution(vec![
            ResolverAttempt::success("cloudflare", vec![global]),
            ResolverAttempt::success("google", vec![global]),
            ResolverAttempt::failed("dnspod", "timeout"),
            ResolverAttempt::failed("alidns", "connect-error"),
        ]);
        assert_eq!(global_fallback.ips, vec![global]);
    }

    #[test]
    fn a_single_resolver_is_enough_after_official_range_filtering() {
        let prefixes = vec!["104.16.0.0/13".parse().expect("valid prefix")];
        let cloudflare = "104.18.26.94".parse().expect("valid IPv4");
        let fake = "28.0.2.55".parse().expect("valid IPv4");
        assert_eq!(
            verified_cloudflare_ips(HashSet::from([cloudflare, fake]), &prefixes),
            vec![cloudflare]
        );
        let resolution = finalize_resolution(vec![
            ResolverAttempt::failed("cloudflare", "timeout"),
            ResolverAttempt::failed("google", "timeout"),
            ResolverAttempt::success("dnspod", vec![cloudflare]),
            ResolverAttempt::failed("alidns", "timeout"),
        ]);
        assert_eq!(resolution.ips, vec![cloudflare]);
    }

    #[test]
    fn complete_resolver_failure_is_reported_without_claiming_censorship() {
        let resolution = finalize_resolution(vec![
            ResolverAttempt::failed("cloudflare", "timeout"),
            ResolverAttempt::failed("google", "connect-error"),
            ResolverAttempt::failed("dnspod", "transport-error"),
            ResolverAttempt::failed("alidns", "timeout"),
        ]);
        assert!(resolution.ips.is_empty());
        let summary = resolution.all_failed_summary().expect("failure summary");
        assert!(summary.contains("cloudflare=timeout"));
        assert!(!summary.to_ascii_lowercase().contains("blocked"));
    }

    #[test]
    fn diagnostics_distinguish_healthy_degraded_and_unavailable_resolvers() {
        let attempts = vec![
            ResolverAttempt::success("cloudflare", Vec::new()),
            ResolverAttempt::success("google", Vec::new()),
            ResolverAttempt::failed("google", "timeout"),
            ResolverAttempt::failed("dnspod", "connect-error"),
        ];
        let diagnostics = aggregate_resolver_diagnostics(&attempts);
        assert_eq!(diagnostics[0].status, "healthy");
        assert_eq!(diagnostics[1].status, "degraded");
        assert_eq!(diagnostics[2].status, "unavailable");
        assert_eq!(
            diagnostics[2].last_error_code.as_deref(),
            Some("connect-error")
        );
    }

    #[test]
    fn fallback_paths_distinguish_doh_ranges_and_unavailability() {
        assert_eq!(initial_resolution_path(true, true), "multi-doh");
        assert_eq!(initial_resolution_path(false, true), "official-ranges");
        assert_eq!(initial_resolution_path(false, false), "unavailable");
    }
}
