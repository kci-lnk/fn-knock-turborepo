use super::*;
use crate::net_utils::{ipv4_prefix_len, ipv6_prefix_len};
use hickory_resolver::{
    Resolver, TokioResolver,
    config::{ConnectionConfig, LookupIpStrategy, NameServerConfig, ResolveHosts, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
};

pub(super) const PUBLIC_DNS_ALIDNS: [&str; 4] = [
    "223.5.5.5",
    "223.6.6.6",
    "2400:3200::1",
    "2400:3200:baba::1",
];
pub(super) const PUBLIC_DNS_TENCENT: [&str; 4] = [
    "119.29.29.29",
    "182.254.116.116",
    "2402:4e00::",
    "2402:4e00:1::",
];
pub(super) const PUBLIC_DNS_CLOUDFLARE: [&str; 4] = [
    "1.1.1.1",
    "1.0.0.1",
    "2606:4700:4700::1111",
    "2606:4700:4700::1001",
];
pub(super) const PUBLIC_DNS_GOOGLE: [&str; 4] = [
    "8.8.8.8",
    "8.8.4.4",
    "2001:4860:4860::8888",
    "2001:4860:4860::8844",
];

pub(super) async fn test_public_check_sources_inner(
    sources: &Value,
    transport: &str,
    public_dns_provider: &str,
    network_interface: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<Vec<Value>> {
    let mut tasks = JoinSet::new();
    let mut index = 0_usize;
    for (family, version) in [("ipv4", 4_u8), ("ipv6", 6_u8)] {
        let urls = sources
            .get(family)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for url in urls
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
        {
            let transport = transport.to_string();
            let public_dns_provider = public_dns_provider.to_string();
            let network_interface = network_interface.map(str::to_string);
            let translator = translator.clone();
            let current_index = index;
            index += 1;
            tasks.spawn(async move {
                let result = test_single_public_check_source(
                    &url,
                    family,
                    version,
                    &transport,
                    &public_dns_provider,
                    network_interface.as_deref(),
                    &translator,
                )
                .await;
                (current_index, result)
            });
        }
    }
    let mut results = vec![Value::Null; index];
    while let Some(result) = tasks.join_next().await {
        let (index, value) = result?;
        if let Some(slot) = results.get_mut(index) {
            *slot = value;
        }
    }
    Ok(results)
}

#[derive(Clone, Debug, Default)]
pub(super) struct CurrentPublicIps {
    pub(super) ipv4: Option<String>,
    pub(super) ipv6: Option<String>,
    pub(super) ipv4_error: Option<String>,
    pub(super) ipv6_error: Option<String>,
}

pub(super) async fn detect_current_public_ips(
    sources: &Value,
    transport: &str,
    public_dns_provider: &str,
    network_interface: Option<&str>,
    enable_ipv4: bool,
    enable_ipv6: bool,
    translator: &Translator,
) -> CurrentPublicIps {
    let ipv4_sources = public_check_source_urls(sources, "ipv4");
    let ipv6_sources = public_check_source_urls(sources, "ipv6");
    let network_interface = normalize_network_interface(network_interface);
    let transport =
        normalize_http_transport(Some(&Value::String(transport.to_string()))).to_string();
    let public_dns_provider = normalize_public_dns_provider(Some(public_dns_provider)).to_string();
    let translator_ipv4 = translator.clone();
    let translator_ipv6 = translator.clone();
    let ipv4_interface = network_interface.clone();
    let ipv6_interface = network_interface.clone();
    let ipv4_transport = transport.clone();
    let ipv6_transport = transport.clone();
    let ipv4_public_dns_provider = public_dns_provider.clone();
    let ipv6_public_dns_provider = public_dns_provider.clone();

    let (ipv4, ipv6) = tokio::join!(
        async move {
            if enable_ipv4 {
                detect_public_ip_family(
                    ipv4_sources,
                    "ipv4",
                    4,
                    ipv4_transport,
                    ipv4_public_dns_provider,
                    ipv4_interface,
                    translator_ipv4,
                )
                .await
            } else {
                PublicIpFamilyDetection::default()
            }
        },
        async move {
            if enable_ipv6 {
                detect_public_ip_family(
                    ipv6_sources,
                    "ipv6",
                    6,
                    ipv6_transport,
                    ipv6_public_dns_provider,
                    ipv6_interface,
                    translator_ipv6,
                )
                .await
            } else {
                PublicIpFamilyDetection::default()
            }
        }
    );

    CurrentPublicIps {
        ipv4: ipv4.ip,
        ipv6: ipv6.ip,
        ipv4_error: ipv4.error,
        ipv6_error: ipv6.error,
    }
}

#[derive(Clone, Debug, Default)]
struct PublicIpFamilyDetection {
    ip: Option<String>,
    error: Option<String>,
}

async fn detect_public_ip_family(
    sources: Vec<String>,
    family: &'static str,
    version: u8,
    transport: String,
    public_dns_provider: String,
    network_interface: String,
    translator: Translator,
) -> PublicIpFamilyDetection {
    if sources.is_empty() {
        return PublicIpFamilyDetection {
            ip: None,
            error: Some(ddns_text(
                &translator,
                "publicCheckSourceListEmpty",
                &[(
                    "family",
                    if version == 4 { "IPv4" } else { "IPv6" }.to_string(),
                )],
            )),
        };
    }

    let mut tasks = JoinSet::new();
    for url in sources {
        let transport = transport.clone();
        let public_dns_provider = public_dns_provider.clone();
        let network_interface = network_interface.clone();
        let translator = translator.clone();
        tasks.spawn(async move {
            test_single_public_check_source(
                &url,
                family,
                version,
                &transport,
                &public_dns_provider,
                Some(network_interface.as_str()),
                &translator,
            )
            .await
        });
    }

    let mut failures = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(value) => {
                if value.get("success").and_then(Value::as_bool) == Some(true)
                    && let Some(ip) = value.get("ip").and_then(Value::as_str)
                {
                    tasks.abort_all();
                    return PublicIpFamilyDetection {
                        ip: Some(ip.to_string()),
                        error: None,
                    };
                }
                if let Some(error) = value.get("error").and_then(Value::as_str) {
                    failures.push(error.to_string());
                }
            }
            Err(error) => failures.push(error.to_string()),
        }
    }

    PublicIpFamilyDetection {
        ip: None,
        error: Some(
            failures
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("; "),
        ),
    }
}

fn public_check_source_urls(sources: &Value, family: &str) -> Vec<String> {
    sources
        .get(family)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect()
}

pub(super) async fn test_single_public_check_source(
    url: &str,
    family: &str,
    version: u8,
    transport: &str,
    public_dns_provider: &str,
    network_interface: Option<&str>,
    translator: &Translator,
) -> Value {
    let result = if normalize_http_transport(Some(&Value::String(transport.to_string()))) == "node"
    {
        test_single_public_check_source_via_reqwest(
            url,
            version,
            public_dns_provider,
            network_interface,
            translator,
        )
        .await
    } else {
        test_single_public_check_source_via_curl(
            url,
            version,
            public_dns_provider,
            network_interface,
            translator,
        )
        .await
    };

    match result {
        Ok((status, text)) => {
            public_check_result_from_response(url, family, version, status, &text, translator)
        }
        Err(error) => json!({
            "family": family,
            "url": url,
            "success": false,
            "status": null,
            "ip": null,
            "error": error.to_string()
        }),
    }
}

async fn test_single_public_check_source_via_reqwest(
    url: &str,
    version: u8,
    public_dns_provider: &str,
    network_interface: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<(u16, String)> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_millis(IP_DETECTION_TIMEOUT_MS))
        .redirect(reqwest::redirect::Policy::limited(20))
        .no_proxy();
    let interface = normalize_network_interface(network_interface);
    let public_dns_provider = normalize_public_dns_provider(Some(public_dns_provider));
    if public_dns_provider != "none" {
        builder = builder.dns_resolver(build_public_dns_resolver(
            public_dns_provider,
            version,
            Some(interface.as_str()),
            translator,
        )?);
    }
    if !interface.is_empty() && !interface.starts_with(DOCKER_HOST_INTERFACE_PREFIX) {
        let local_address = first_selectable_interface_ip(&interface, version, translator)?
            .ok_or_else(|| {
                anyhow::anyhow!(ddns_text(
                    translator,
                    "nodeTransportInterfaceAddressUnavailable",
                    &[
                        ("name", interface.clone()),
                        (
                            "family",
                            if version == 4 { "IPv4" } else { "IPv6" }.to_string(),
                        ),
                    ],
                ))
            })?;
        builder = builder.local_address(local_address);
    } else if public_dns_provider == "none" {
        builder = apply_reqwest_family_resolver(builder, url, version, translator).await?;
    }
    let response = builder
        .build()?
        .get(url)
        .header("Accept", "application/json, text/plain")
        .send()
        .await
        .map_err(|error| anyhow::anyhow!(deepest_error_message(&error)))?;
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    Ok((status, text))
}

async fn test_single_public_check_source_via_curl(
    url: &str,
    version: u8,
    public_dns_provider: &str,
    network_interface: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<(u16, String)> {
    let public_dns_provider = normalize_public_dns_provider(Some(public_dns_provider));
    if public_dns_provider == "none" {
        let (status, body, _) = run_curl_public_check_request(
            url,
            version,
            network_interface,
            translator,
            &[],
            true,
            Duration::from_millis(IP_DETECTION_TIMEOUT_MS),
        )
        .await?;
        return Ok((status, body));
    }

    let resolver =
        build_public_dns_resolver(public_dns_provider, version, network_interface, translator)?;
    let deadline = tokio_time::Instant::now() + Duration::from_millis(IP_DETECTION_TIMEOUT_MS);
    let mut current_url = url.to_string();
    for redirect_count in 0..=20 {
        let resolve_entries = await_with_public_check_deadline(
            deadline,
            curl_resolve_entries(&current_url, &resolver),
            translator,
        )
        .await?;
        let timeout = remaining_public_check_timeout(deadline, translator)?;
        let (status, body, redirect_url) = run_curl_public_check_request(
            &current_url,
            version,
            network_interface,
            translator,
            &resolve_entries,
            false,
            timeout,
        )
        .await?;
        if (300..400).contains(&status) && !redirect_url.is_empty() {
            if redirect_count == 20 {
                anyhow::bail!(
                    "{}",
                    ddns_text(translator, "publicCheckTooManyRedirects", &[])
                );
            }
            current_url = redirect_url;
            continue;
        }
        return Ok((status, body));
    }
    unreachable!("redirect loop always returns or errors")
}

pub(super) fn deepest_error_message(error: &(dyn std::error::Error + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message = cause.to_string();
        source = cause.source();
    }
    message
}

pub(super) fn remaining_public_check_timeout(
    deadline: tokio_time::Instant,
    translator: &Translator,
) -> anyhow::Result<Duration> {
    match deadline.checked_duration_since(tokio_time::Instant::now()) {
        Some(remaining) if !remaining.is_zero() => Ok(remaining),
        _ => anyhow::bail!("{}", ddns_text(translator, "publicCheckTimeout", &[])),
    }
}

pub(super) async fn await_with_public_check_deadline<T, F>(
    deadline: tokio_time::Instant,
    future: F,
    translator: &Translator,
) -> anyhow::Result<T>
where
    F: Future<Output = anyhow::Result<T>>,
{
    let timeout = remaining_public_check_timeout(deadline, translator)?;
    tokio_time::timeout(timeout, future)
        .await
        .map_err(|_| anyhow::anyhow!(ddns_text(translator, "publicCheckTimeout", &[])))?
}

async fn run_curl_public_check_request(
    url: &str,
    version: u8,
    network_interface: Option<&str>,
    translator: &Translator,
    resolve_entries: &[String],
    follow_redirects: bool,
    timeout: Duration,
) -> anyhow::Result<(u16, String, String)> {
    const STATUS_MARKER: &str = "\n__FN_KNOCK_CURL_STATUS__";
    const REDIRECT_MARKER: &str = "\n__FN_KNOCK_CURL_REDIRECT__";
    const PROXY_ENV_KEYS: [&str; 8] = [
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "no_proxy",
        "NO_PROXY",
    ];

    let mut command = tokio::process::Command::new("curl");
    command
        .arg("-q")
        .arg("--silent")
        .arg("--show-error")
        .arg(if version == 4 { "-4" } else { "-6" })
        .arg("--max-time")
        .arg(format!("{:.3}", timeout.as_secs_f64().max(0.001)))
        .arg("--write-out")
        .arg(format!(
            "{STATUS_MARKER}%{{http_code}}{REDIRECT_MARKER}%{{redirect_url}}"
        ))
        .arg("--header")
        .arg("Accept: application/json, text/plain")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if follow_redirects {
        command.arg("--location");
    }
    for key in PROXY_ENV_KEYS {
        command.env_remove(key);
    }
    let interface = normalize_network_interface(network_interface);
    if !interface.is_empty() && !interface.starts_with(DOCKER_HOST_INTERFACE_PREFIX) {
        ensure_ddns_network_interface_exists(&interface, translator)?;
        command.arg("--interface").arg(interface);
    }
    for entry in resolve_entries {
        command.arg("--resolve").arg(entry);
    }
    command.arg(url);
    let output = command.output().await?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "curlRequestFailed",
                &[(
                    "detail",
                    if detail.is_empty() {
                        output
                            .status
                            .code()
                            .map(|code| format!("exit {code}"))
                            .unwrap_or_else(|| "terminated".to_string())
                    } else {
                        detail
                    },
                )],
            )
        );
    }
    let output_text = String::from_utf8_lossy(&output.stdout).to_string();
    let Some((before_redirect, redirect_url)) = output_text.rsplit_once(REDIRECT_MARKER) else {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "curlRequestFailed",
                &[("detail", "missing redirect marker".to_string())],
            )
        );
    };
    let Some((body, status_text)) = before_redirect.rsplit_once(STATUS_MARKER) else {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "curlRequestFailed",
                &[("detail", "missing status".to_string())],
            )
        );
    };
    let status = status_text.trim().parse::<u16>().unwrap_or(0);
    Ok((status, body.to_string(), redirect_url.trim().to_string()))
}

#[derive(Clone)]
struct PublicDnsResolver {
    inner: TokioResolver,
    version: u8,
    translator: Translator,
}

impl PublicDnsResolver {
    async fn resolve_host(&self, host: &str) -> anyhow::Result<Vec<IpAddr>> {
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(vec![ip]);
        }
        let query = format!("{}.", host.trim_end_matches('.'));
        let lookup = self.inner.lookup_ip(query).await.map_err(|error| {
            anyhow::anyhow!(ddns_text(
                &self.translator,
                "publicDnsResolveFailed",
                &[
                    ("host", host.to_string()),
                    (
                        "family",
                        if self.version == 4 { "IPv4" } else { "IPv6" }.to_string(),
                    ),
                    ("detail", error.to_string()),
                ],
            ))
        })?;
        let addresses = lookup
            .iter()
            .filter(|ip| (self.version == 4 && ip.is_ipv4()) || (self.version == 6 && ip.is_ipv6()))
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            anyhow::bail!(
                "{}",
                ddns_text(
                    &self.translator,
                    "publicDnsNoAddress",
                    &[
                        ("host", host.to_string()),
                        (
                            "family",
                            if self.version == 4 { "IPv4" } else { "IPv6" }.to_string(),
                        ),
                    ],
                )
            );
        }
        Ok(addresses)
    }
}

impl reqwest::dns::Resolve for PublicDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolver = self.clone();
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addresses = resolver.resolve_host(&host).await.map_err(|error| {
                Box::new(std::io::Error::other(error.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;
            Ok(Box::new(
                addresses
                    .into_iter()
                    .map(|ip| std::net::SocketAddr::new(ip, 0)),
            ) as reqwest::dns::Addrs)
        })
    }
}

pub(super) fn public_dns_server_addresses(provider: &str) -> &'static [&'static str] {
    match normalize_public_dns_provider(Some(provider)) {
        "tencent" => &PUBLIC_DNS_TENCENT,
        "cloudflare" => &PUBLIC_DNS_CLOUDFLARE,
        "google" => &PUBLIC_DNS_GOOGLE,
        "alidns" => &PUBLIC_DNS_ALIDNS,
        _ => &[],
    }
}

fn build_public_dns_resolver(
    provider: &str,
    version: u8,
    network_interface: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<PublicDnsResolver> {
    let interface = normalize_network_interface(network_interface);
    let should_bind = !interface.is_empty() && !interface.starts_with(DOCKER_HOST_INTERFACE_PREFIX);
    let local_ipv4 = if should_bind {
        first_selectable_interface_ip(&interface, 4, translator)?
    } else {
        None
    };
    let local_ipv6 = if should_bind {
        first_selectable_interface_ip(&interface, 6, translator)?
    } else {
        None
    };
    let mut name_servers = Vec::new();
    for address in public_dns_server_addresses(provider) {
        let ip = address.parse::<IpAddr>()?;
        let bind_ip = if ip.is_ipv4() { local_ipv4 } else { local_ipv6 };
        if should_bind && bind_ip.is_none() {
            continue;
        }
        let bind_addr = bind_ip.map(|ip| std::net::SocketAddr::new(ip, 0));
        let mut udp = ConnectionConfig::udp();
        udp.bind_addr = bind_addr;
        let mut tcp = ConnectionConfig::tcp();
        tcp.bind_addr = bind_addr;
        name_servers.push(NameServerConfig::new(ip, true, vec![udp, tcp]));
    }
    if name_servers.is_empty() {
        anyhow::bail!("{}", ddns_text(translator, "publicDnsNoUsableServer", &[]));
    }
    let config = ResolverConfig::from_parts(None, Vec::new(), name_servers);
    let mut builder = Resolver::builder_with_config(config, TokioRuntimeProvider::default());
    let options = builder.options_mut();
    options.timeout = Duration::from_millis(IP_DETECTION_TIMEOUT_MS);
    options.attempts = 1;
    options.num_concurrent_reqs = 4;
    options.try_tcp_on_error = true;
    options.use_hosts_file = ResolveHosts::Never;
    options.ip_strategy = if version == 4 {
        LookupIpStrategy::Ipv4Only
    } else {
        LookupIpStrategy::Ipv6Only
    };
    Ok(PublicDnsResolver {
        inner: builder.build()?,
        version,
        translator: translator.clone(),
    })
}

async fn curl_resolve_entries(
    url: &str,
    resolver: &PublicDnsResolver,
) -> anyhow::Result<Vec<String>> {
    let parsed = Url::parse(url)?;
    let Some(host) = parsed.host_str() else {
        return Ok(Vec::new());
    };
    if host.parse::<IpAddr>().is_ok() {
        return Ok(Vec::new());
    }
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("missing URL port"))?;
    Ok(resolver
        .resolve_host(host)
        .await?
        .into_iter()
        .map(|ip| format_curl_resolve_entry(host, port, ip))
        .collect())
}

pub(super) fn format_curl_resolve_entry(host: &str, port: u16, ip: IpAddr) -> String {
    if ip.is_ipv6() {
        format!("{host}:{port}:[{ip}]")
    } else {
        format!("{host}:{port}:{ip}")
    }
}

async fn apply_reqwest_family_resolver(
    builder: reqwest::ClientBuilder,
    url: &str,
    version: u8,
    translator: &Translator,
) -> anyhow::Result<reqwest::ClientBuilder> {
    let parsed = Url::parse(url)?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "nodeTransportUnsupportedProtocol",
                &[("protocol", format!("{}:", parsed.scheme()))],
            )
        );
    }
    let Some(host) = parsed.host_str() else {
        return Ok(builder);
    };
    let Some(port) = parsed.port_or_known_default() else {
        return Ok(builder);
    };
    let addrs = lookup_host((host, port))
        .await?
        .filter(|addr| {
            (version == 4 && addr.ip().is_ipv4()) || (version == 6 && addr.ip().is_ipv6())
        })
        .collect::<Vec<_>>();
    if addrs.is_empty() {
        anyhow::bail!(
            "no {} address found for {host}",
            if version == 4 { "IPv4" } else { "IPv6" }
        );
    }
    Ok(builder.resolve_to_addrs(host, &addrs))
}

fn ensure_ddns_network_interface_exists(
    interface: &str,
    translator: &Translator,
) -> anyhow::Result<()> {
    if list_ddns_network_interfaces()
        .iter()
        .any(|item| item.get("name").and_then(Value::as_str) == Some(interface))
    {
        Ok(())
    } else {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "interfaceNotFound",
                &[("name", interface.to_string())],
            )
        )
    }
}

fn first_selectable_interface_ip(
    interface: &str,
    version: u8,
    translator: &Translator,
) -> anyhow::Result<Option<IpAddr>> {
    ensure_ddns_network_interface_exists(interface, translator)?;
    Ok(list_ddns_network_interfaces()
        .into_iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(interface))
        .and_then(|item| first_interface_ip_from_option(&item, version)))
}

pub(super) fn first_interface_ip_from_option(item: &Value, version: u8) -> Option<IpAddr> {
    item.get("addresses")
        .and_then(Value::as_array)
        .cloned()
        .into_iter()
        .flatten()
        .filter(|item| {
            item.get("family").and_then(Value::as_str)
                == Some(if version == 4 { "ipv4" } else { "ipv6" })
        })
        .find_map(|item| {
            item.get("address")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<IpAddr>().ok())
        })
}

fn public_check_result_from_response(
    url: &str,
    family: &str,
    version: u8,
    status: u16,
    text: &str,
    translator: &Translator,
) -> Value {
    let preview = response_preview(text);
    if !(200..300).contains(&status) {
        return json!({
            "family": family,
            "url": url,
            "success": false,
            "status": status,
            "ip": null,
            "responsePreview": preview,
            "error": public_check_request_failed_message(translator, url, status)
        });
    }
    let ip = parse_detected_ip_text(text, version);
    if let Some(ip) = ip {
        json!({
            "family": family,
            "url": url,
            "success": true,
            "status": status,
            "ip": ip,
            "responsePreview": preview
        })
    } else {
        json!({
            "family": family,
            "url": url,
            "success": false,
            "status": status,
            "ip": null,
            "responsePreview": preview,
            "error": public_check_invalid_payload_message(translator, url, version)
        })
    }
}

pub(super) fn public_check_request_failed_message(
    translator: &Translator,
    url: &str,
    status: u16,
) -> String {
    ddns_text(
        translator,
        "publicCheckSourceRequestFailed",
        &[("url", url.to_string()), ("status", status.to_string())],
    )
}

pub(super) fn public_check_invalid_payload_message(
    translator: &Translator,
    url: &str,
    version: u8,
) -> String {
    ddns_text(
        translator,
        "publicCheckSourceInvalidPayload",
        &[
            ("url", url.to_string()),
            (
                "family",
                if version == 4 { "IPv4" } else { "IPv6" }.to_string(),
            ),
        ],
    )
}

pub(super) fn parse_detected_ip_text(text: &str, version: u8) -> Option<String> {
    parse_detected_ip(text.trim(), version).or_else(|| {
        let value = serde_json::from_str::<Value>(text).ok()?;
        if let Some(ip) = value.get("ip").and_then(Value::as_str) {
            return parse_detected_ip(ip, version);
        }
        if let Some(ip) = value.get("address").and_then(Value::as_str) {
            return parse_detected_ip(ip, version);
        }
        value
            .as_str()
            .and_then(|value| parse_detected_ip(value, version))
    })
}

pub(super) fn parse_detected_ip(value: &str, version: u8) -> Option<String> {
    let ip = value.trim().parse::<IpAddr>().ok()?;
    match (version, ip) {
        (4, IpAddr::V4(_)) | (6, IpAddr::V6(_)) => Some(value.trim().to_string()),
        _ => None,
    }
}

pub(super) fn response_preview(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() > RESPONSE_PREVIEW_MAX_LENGTH {
        format!("{}...", &normalized[..RESPONSE_PREVIEW_MAX_LENGTH])
    } else {
        normalized
    }
}

pub(super) fn list_ddns_network_interfaces() -> Vec<Value> {
    let mut interfaces = list_docker_host_ipv6_interfaces();
    let mut runtime = HashMap::<String, Vec<Value>>::new();
    let runtime_ipv6_metadata = read_local_if_inet6_metadata();
    if let Ok(addrs) = get_if_addrs() {
        for iface in addrs {
            if iface.is_loopback() {
                continue;
            }
            let address = match iface.addr {
                IfAddr::V4(addr) if is_usable_ipv4(addr.ip) => json!({
                    "family": "ipv4",
                    "address": addr.ip.to_string(),
                    "cidr": format!("{}/{}", addr.ip, ipv4_prefix_len(addr.netmask)),
                    "prefixLength": ipv4_prefix_len(addr.netmask),
                    "internal": false,
                    "source": "runtime",
                    "temporary": Value::Null,
                    "deprecated": Value::Null,
                    "tentative": Value::Null,
                    "dadFailed": Value::Null
                }),
                IfAddr::V6(addr) if is_usable_ipv6(addr.ip) => {
                    let address = addr.ip.to_string();
                    let status = runtime_ipv6_metadata.get(&(iface.name.clone(), address.clone()));
                    json!({
                        "family": "ipv6",
                        "address": address,
                        "cidr": format!("{}/{}", addr.ip, ipv6_prefix_len(addr.netmask)),
                        "prefixLength": ipv6_prefix_len(addr.netmask),
                        "internal": false,
                        "source": "runtime",
                        "temporary": status.and_then(|item| item.get("temporary")).cloned().unwrap_or(Value::Null),
                        "deprecated": status.and_then(|item| item.get("deprecated")).cloned().unwrap_or(Value::Null),
                        "tentative": status.and_then(|item| item.get("tentative")).cloned().unwrap_or(Value::Null),
                        "dadFailed": status.and_then(|item| item.get("dadFailed")).cloned().unwrap_or(Value::Null)
                    })
                }
                _ => continue,
            };
            runtime.entry(iface.name).or_default().push(address);
        }
    }

    let mut runtime_items = runtime
        .into_iter()
        .filter_map(|(name, addresses)| interface_option(&name, "runtime", addresses))
        .collect::<Vec<_>>();
    runtime_items.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    interfaces.extend(runtime_items);
    interfaces.sort_by(|left, right| {
        let left_source = left
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("runtime");
        let right_source = right
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("runtime");
        if left_source != right_source {
            return if left_source == "docker_host" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    interfaces
}

pub(super) fn interface_option(name: &str, source: &str, addresses: Vec<Value>) -> Option<Value> {
    if addresses.is_empty() {
        return None;
    }
    let selectable = addresses
        .iter()
        .filter(|item| is_selectable_interface_address(item))
        .cloned()
        .collect::<Vec<_>>();
    let summary = addresses
        .iter()
        .filter_map(|item| {
            let family = item.get("family").and_then(Value::as_str)?;
            let address = item.get("address").and_then(Value::as_str)?;
            Some(format!(
                "{}: {}",
                if family == "ipv4" { "IPv4" } else { "IPv6" },
                address
            ))
        })
        .collect::<Vec<_>>()
        .join(" / ");
    if selectable.is_empty() && source == "docker_host" {
        return None;
    }
    Some(json!({
        "name": name,
        "label": format!("{name} ({summary})"),
        "summary": summary,
        "source": source,
        "hasIpv4": addresses.iter().any(|item| item.get("family").and_then(Value::as_str) == Some("ipv4")),
        "hasIpv6": addresses.iter().any(|item| item.get("family").and_then(Value::as_str) == Some("ipv6")),
        "addresses": addresses,
        "selectableAddresses": selectable
    }))
}

pub(super) fn list_docker_host_ipv6_interfaces() -> Vec<Value> {
    let path = env::var("DDNS_HOST_IF_INET6_PATH")
        .unwrap_or_else(|_| DEFAULT_DOCKER_HOST_IF_INET6_PATH.to_string());
    fs::read_to_string(path)
        .ok()
        .map(|content| parse_host_if_inet6(&content))
        .unwrap_or_default()
}

pub(super) fn parse_host_if_inet6(content: &str) -> Vec<Value> {
    let mut by_interface = HashMap::<String, Vec<Value>>::new();
    for line in content.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 6 {
            continue;
        }
        let Some(address) = format_ipv6_from_proc_hex(parts[0]) else {
            continue;
        };
        let prefix_len = u8::from_str_radix(parts[2], 16).unwrap_or(0);
        let scope = u8::from_str_radix(parts[3], 16).unwrap_or(255);
        let flags = u32::from_str_radix(parts[4], 16).unwrap_or(0);
        if scope != 0 {
            continue;
        }
        let Ok(ip) = address.parse::<Ipv6Addr>() else {
            continue;
        };
        if !is_usable_ipv6(ip) {
            continue;
        }
        let name = parts[5].to_string();
        let status = ipv6_status_from_flags(flags);
        by_interface.entry(name).or_default().push(json!({
            "family": "ipv6",
            "address": address,
            "cidr": format!("{address}/{prefix_len}"),
            "prefixLength": prefix_len,
            "internal": false,
            "source": "docker_host",
            "temporary": status["temporary"],
            "deprecated": status["deprecated"],
            "tentative": status["tentative"],
            "dadFailed": status["dadFailed"]
        }));
    }
    let mut items = by_interface
        .into_iter()
        .filter_map(|(name, mut addresses)| {
            addresses.sort_by(|left, right| {
                left.get("address")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .cmp(right.get("address").and_then(Value::as_str).unwrap_or(""))
            });
            interface_option(
                &format!("{DOCKER_HOST_INTERFACE_PREFIX}{name}"),
                "docker_host",
                addresses,
            )
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    items
}

pub(super) fn read_local_if_inet6_metadata() -> HashMap<(String, String), Value> {
    fs::read_to_string("/proc/net/if_inet6")
        .ok()
        .map(|content| parse_if_inet6_metadata(&content))
        .unwrap_or_default()
}

pub(super) fn parse_if_inet6_metadata(content: &str) -> HashMap<(String, String), Value> {
    let mut output = HashMap::new();
    for line in content.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 6 {
            continue;
        }
        let Some(address) = format_ipv6_from_proc_hex(parts[0]) else {
            continue;
        };
        let flags = u32::from_str_radix(parts[4], 16).unwrap_or(0);
        output.insert(
            (parts[5].to_string(), address),
            ipv6_status_from_flags(flags),
        );
    }
    output
}

pub(super) fn ipv6_status_from_flags(flags: u32) -> Value {
    json!({
        "temporary": flags & 0x01 != 0,
        "dadFailed": flags & 0x08 != 0,
        "deprecated": flags & 0x20 != 0,
        "tentative": flags & 0x40 != 0
    })
}

pub(super) fn format_ipv6_from_proc_hex(value: &str) -> Option<String> {
    if value.len() != 32 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let mut segments = Vec::new();
    for chunk in value.as_bytes().chunks(4) {
        let raw = std::str::from_utf8(chunk).ok()?;
        segments.push(u16::from_str_radix(raw, 16).ok()?);
    }
    Some(
        Ipv6Addr::new(
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7],
        )
        .to_string(),
    )
}

pub(super) fn is_selectable_interface_address(value: &Value) -> bool {
    let Some(address) = value.get("address").and_then(Value::as_str) else {
        return false;
    };
    match value.get("family").and_then(Value::as_str) {
        Some("ipv4") => address.parse::<Ipv4Addr>().is_ok_and(is_global_ipv4),
        Some("ipv6") => address.parse::<Ipv6Addr>().is_ok_and(is_global_ipv6),
        _ => false,
    }
}

pub(super) fn is_global_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_unspecified()
        || ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || octets[0] == 0
        || octets[0] >= 240
        || matches!(octets, [100, second, _, _] if (64..=127).contains(&second))
        || matches!(octets, [192, 0, 0, _] | [192, 88, 99, _])
        || matches!(octets, [198, second, _, _] if second == 18 || second == 19))
}

pub(super) fn is_global_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    segments[0] & 0xe000 == 0x2000
        && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
        && !(segments[0] == 0x3fff && segments[1] & 0xf000 == 0)
}

pub(super) fn is_usable_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] != 127 && !(octets[0] == 169 && octets[1] == 254)
}

pub(super) fn is_usable_ipv6(ip: Ipv6Addr) -> bool {
    !(ip.is_loopback() || ip.is_unicast_link_local() || ip.is_unspecified())
}
