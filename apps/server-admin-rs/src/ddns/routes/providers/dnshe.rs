use super::*;

const DNSHE_API_URL: &str = "https://api005.dnshe.com/index.php";
const DNSHE_SUBDOMAIN_PAGE_SIZE: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ddns::routes) struct DnsheSubdomainMatch {
    pub(in crate::ddns::routes) id: i64,
    pub(in crate::ddns::routes) status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ddns::routes) struct DnsheRecordMatch {
    pub(in crate::ddns::routes) id: i64,
    pub(in crate::ddns::routes) content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ddns::routes) enum DnsheRecordLookup {
    Missing,
    MissingId,
    Found(DnsheRecordMatch),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ddns::routes) enum DnsheRecordUpdatePlan {
    Noop,
    Update(i64),
    Create,
    MissingId,
}

#[derive(Clone)]
struct DnsheClient {
    translator: Translator,
    http: DDNSHttpClient,
    api_key: String,
    api_secret: String,
}

pub(in crate::ddns::routes) struct DnsheRequestSpec {
    pub(in crate::ddns::routes) method: reqwest::Method,
    pub(in crate::ddns::routes) url: String,
    pub(in crate::ddns::routes) headers: Vec<(String, String)>,
    pub(in crate::ddns::routes) body: Option<Value>,
}

impl DnsheClient {
    fn new(
        translator: &Translator,
        http: DDNSHttpClient,
        api_key: String,
        api_secret: String,
    ) -> Self {
        Self {
            translator: translator.clone(),
            http,
            api_key,
            api_secret,
        }
    }

    async fn request(
        &self,
        endpoint: &str,
        action: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let spec = dnshe_request_spec(
            &self.api_key,
            &self.api_secret,
            endpoint,
            action,
            query,
            body,
        )?;
        let mut request = self.http.request(spec.method, spec.url);
        for (name, value) in spec.headers {
            request = request.header(name, value);
        }
        if let Some(body) = spec.body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| provider_request_error(&self.translator, "dnshe", error))?;
        let (status, data, _) = response_json(&self.translator, response)
            .await
            .map_err(|error| provider_request_error(&self.translator, "dnshe", error))?;
        assert_dnshe_success(status, &data).map_err(|error| {
            anyhow::anyhow!(ddns_text(
                &self.translator,
                "providers.dnshe.apiError",
                &[("detail", error.to_string())],
            ))
        })?;
        Ok(data)
    }
}

pub(in crate::ddns::routes) fn dnshe_catalog_entry() -> Value {
    provider(
        "dnshe",
        "DNSHE",
        vec![
            field("api_key", "API Key", "text", "DNSHE API Key", true),
            field(
                "api_secret",
                "API Secret",
                "password",
                "DNSHE API Secret",
                true,
            ),
            field(
                "root_domain",
                "DNSHE Managed Domain",
                "text",
                "example.com",
                true,
            ),
            field("domain", "Domain", "text", "home.example.com", true),
            field("ttl", "TTL", "text", "600", false),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_dnshe(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_key = config_value(config, "api_key");
    let api_secret = config_value(config, "api_secret");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if api_key.is_empty() || api_secret.is_empty() || root_domain.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dnshe.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dnshe.noIpAvailable",
            &[],
        )));
    }

    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client_no_redirects(translator, http_options)
        .map_err(|error| provider_request_error(translator, "dnshe", error))?;
    let dnshe = DnsheClient::new(translator, client, api_key, api_secret);
    let subdomain = resolve_dnshe_subdomain(&dnshe, &parsed.root_domain).await?;
    let records = dnshe
        .request(
            "dns_records",
            "list",
            &[("subdomain_id", subdomain.id.to_string())],
            None,
        )
        .await?;
    let ttl = positive_i64(config.get("ttl"), 600);
    let provider_label_text = provider_label(Some("dnshe"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let dnshe = dnshe.clone();
            let root_domain = parsed.root_domain.clone();
            let fqdn = parsed.fqdn.clone();
            // The production API rejects FQDNs here and accepts only @ or a relative name.
            let create_name = dnshe_create_record_name(&parsed);
            let records = records.clone();
            async move {
                match plan_dnshe_record_update(&records, &fqdn, &root_domain, record_type, &ip) {
                    DnsheRecordUpdatePlan::Noop => Ok(()),
                    DnsheRecordUpdatePlan::Update(id) => {
                        dnshe
                            .request(
                                "dns_records",
                                "update",
                                &[],
                                Some(json!({
                                    "id": id,
                                    "content": ip,
                                    "ttl": ttl
                                })),
                            )
                            .await?;
                        Ok(())
                    }
                    DnsheRecordUpdatePlan::Create => {
                        dnshe
                            .request(
                                "dns_records",
                                "create",
                                &[],
                                Some(json!({
                                    "subdomain_id": subdomain.id,
                                    "type": record_type,
                                    "name": create_name,
                                    "content": ip,
                                    "ttl": ttl
                                })),
                            )
                            .await?;
                        Ok(())
                    }
                    DnsheRecordUpdatePlan::MissingId => Err(anyhow::anyhow!(ddns_text(
                        translator,
                        "providers.dnshe.recordIdMissing",
                        &[("type", record_type.to_string())],
                    ))),
                }
            }
        },
    )
    .await
}

async fn resolve_dnshe_subdomain(
    client: &DnsheClient,
    root_domain: &str,
) -> anyhow::Result<DnsheSubdomainMatch> {
    let mut page = 1usize;
    loop {
        let data = client
            .request(
                "subdomains",
                "list",
                &[
                    ("page", page.to_string()),
                    ("per_page", DNSHE_SUBDOMAIN_PAGE_SIZE.to_string()),
                    ("fields", "id,full_domain,status".to_string()),
                ],
                None,
            )
            .await?;
        if let Some(found) = find_dnshe_subdomain(&data, root_domain) {
            if dnshe_subdomain_is_usable(&found.status) {
                return Ok(found);
            }
            return Err(anyhow::anyhow!(ddns_text(
                &client.translator,
                "providers.dnshe.managedDomainInactive",
                &[
                    ("domain", normalize_domain(root_domain)),
                    (
                        "status",
                        if found.status.is_empty() {
                            ddns_text(&client.translator, "providers.dnshe.unknownStatus", &[])
                        } else {
                            found.status
                        },
                    ),
                ],
            )));
        }
        let item_count = data
            .get("subdomains")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default();
        if !dnshe_has_more_subdomains(&data, item_count) {
            break;
        }
        page = page.saturating_add(1);
    }

    Err(anyhow::anyhow!(ddns_text(
        &client.translator,
        "providers.dnshe.managedDomainNotFound",
        &[("domain", normalize_domain(root_domain))],
    )))
}

pub(in crate::ddns::routes) fn dnshe_subdomain_is_usable(status: &str) -> bool {
    // Production returns "Registered" for usable managed domains despite documenting "active".
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "active" | "registered"
    )
}

pub(in crate::ddns::routes) fn dnshe_create_record_name(domain: &SplitDomain) -> String {
    domain.record_name.clone()
}

pub(in crate::ddns::routes) fn dnshe_request_spec(
    api_key: &str,
    api_secret: &str,
    endpoint: &str,
    action: &str,
    query: &[(&str, String)],
    body: Option<Value>,
) -> anyhow::Result<DnsheRequestSpec> {
    let url = dnshe_api_url(endpoint, action, query)?;
    let method = if body.is_some() {
        reqwest::Method::POST
    } else {
        reqwest::Method::GET
    };
    let mut headers = vec![
        (
            reqwest::header::ACCEPT.as_str().to_string(),
            "application/json".to_string(),
        ),
        ("X-API-Key".to_string(), api_key.to_string()),
        ("X-API-Secret".to_string(), api_secret.to_string()),
    ];
    if body.is_some() {
        headers.push((
            reqwest::header::CONTENT_TYPE.as_str().to_string(),
            "application/json".to_string(),
        ));
    }
    Ok(DnsheRequestSpec {
        method,
        url,
        headers,
        body,
    })
}

fn dnshe_api_url(endpoint: &str, action: &str, query: &[(&str, String)]) -> anyhow::Result<String> {
    let mut url = Url::parse(DNSHE_API_URL)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("m", "domain_hub")
            .append_pair("endpoint", endpoint)
            .append_pair("action", action);
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

pub(in crate::ddns::routes) fn assert_dnshe_success(
    status: StatusCode,
    data: &Value,
) -> anyhow::Result<()> {
    if status.is_success() && data.get("success").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "[{}] {}",
        status.as_u16(),
        format_dnshe_error(data)
    ))
}

pub(in crate::ddns::routes) fn format_dnshe_error(data: &Value) -> String {
    for key in ["message", "error", "error_code"] {
        if let Some(value) = json_text(data, key) {
            return value;
        }
    }
    "DNSHE API request failed".to_string()
}

pub(in crate::ddns::routes) fn find_dnshe_subdomain(
    data: &Value,
    root_domain: &str,
) -> Option<DnsheSubdomainMatch> {
    let expected = normalize_domain(root_domain).to_ascii_lowercase();
    data.get("subdomains")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|item| {
            let full_domain = normalize_domain(
                item.get("full_domain")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .to_ascii_lowercase();
            if full_domain != expected {
                return None;
            }
            Some(DnsheSubdomainMatch {
                id: read_positive_id(item.get("id"))?,
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            })
        })
}

pub(in crate::ddns::routes) fn dnshe_has_more_subdomains(data: &Value, item_count: usize) -> bool {
    data.pointer("/pagination/has_more")
        .and_then(Value::as_bool)
        .unwrap_or(item_count >= DNSHE_SUBDOMAIN_PAGE_SIZE)
}

pub(in crate::ddns::routes) fn find_dnshe_record(
    data: &Value,
    fqdn: &str,
    root_domain: &str,
    record_type: &str,
) -> DnsheRecordLookup {
    let expected = normalize_domain(fqdn).to_ascii_lowercase();
    let root_domain = normalize_domain(root_domain).to_ascii_lowercase();
    let Some(records) = data.get("records").and_then(Value::as_array) else {
        return DnsheRecordLookup::Missing;
    };
    let Some(record) = records.iter().find(|record| {
        record
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(record_type))
            && dnshe_record_fqdn(
                record
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                &root_domain,
            ) == expected
    }) else {
        return DnsheRecordLookup::Missing;
    };
    let Some(id) = read_positive_id(record.get("id")) else {
        return DnsheRecordLookup::MissingId;
    };
    DnsheRecordLookup::Found(DnsheRecordMatch {
        id,
        content: record
            .get("content")
            .map(value_to_compact_string)
            .unwrap_or_default(),
    })
}

pub(in crate::ddns::routes) fn plan_dnshe_record_update(
    data: &Value,
    fqdn: &str,
    root_domain: &str,
    record_type: &str,
    desired_content: &str,
) -> DnsheRecordUpdatePlan {
    match find_dnshe_record(data, fqdn, root_domain, record_type) {
        DnsheRecordLookup::Missing => DnsheRecordUpdatePlan::Create,
        DnsheRecordLookup::MissingId => DnsheRecordUpdatePlan::MissingId,
        DnsheRecordLookup::Found(record) if record.content == desired_content => {
            DnsheRecordUpdatePlan::Noop
        }
        DnsheRecordLookup::Found(record) => DnsheRecordUpdatePlan::Update(record.id),
    }
}

pub(in crate::ddns::routes) fn dnshe_record_fqdn(name: &str, root_domain: &str) -> String {
    let name = normalize_domain(name).to_ascii_lowercase();
    let root = normalize_domain(root_domain).to_ascii_lowercase();
    if name.is_empty() || name == "@" {
        return root;
    }
    if ddns_domain_is_same_or_subdomain(&name, &root) {
        name
    } else {
        format!("{name}.{root}")
    }
}
