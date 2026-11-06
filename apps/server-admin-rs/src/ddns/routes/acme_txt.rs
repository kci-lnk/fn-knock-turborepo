//! Provider-side identities for the DNS-01 cleanup confirmation. Never invoke a
//! DNS hook's generic remove operation: some hooks clear an entire RRset.
use super::*;
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub(crate) struct TxtRecord {
    pub id: String,
    pub zone: String,
    pub name: String,
    pub value: String,
}

#[derive(Debug, thiserror::Error)]
#[error("DNS API rate limit")]
pub(crate) struct ProviderLimit {
    pub retry_after: Option<String>,
    pub credential_scoped: bool,
}
impl ProviderLimit {
    pub(in crate::ddns::routes) async fn from_response(response: reqwest::Response) -> Self {
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .map(str::to_string);
        let body = crate::http_body::read_response_bytes_limited(response, 32 * 1024)
            .await
            .unwrap_or_default();
        Self::from_body(retry_after, &body)
    }
    pub(crate) fn from_body(retry_after: Option<String>, body: &[u8]) -> Self {
        // Carry only the scope decision, never arbitrary provider error text.
        let text = String::from_utf8_lossy(body).to_ascii_lowercase();
        let credential_scoped = [
            "per account",
            "per token",
            "per api token",
            "per credential",
            "throttling.user",
            "requestlimitexceeded.uinlimitexceeded",
            "requestlimitexceeded.secretidlimitexceeded",
        ]
        .iter()
        .any(|s| text.contains(s));
        Self {
            retry_after,
            credential_scoped,
        }
    }
}

pub(crate) struct TxtProvider<'a> {
    t: &'a Translator,
    kind: &'a str,
    credentials: &'a Value,
    client: DDNSHttpClient,
}

impl<'a> TxtProvider<'a> {
    pub(crate) fn new(t: &'a Translator, kind: &'a str, credentials: &'a Value) -> Result<Self> {
        ensure!(
            matches!(kind, "dns_cf" | "dns_dp" | "dns_tencent" | "dns_ali"),
            "unsupported provider"
        );
        let opts = DDNSHttpClientOptions::from_settings_and_config(&Value::Null, &HashMap::new());
        let mut client = ddns_http_client_no_redirects(t, &opts)?;
        client.report_rate_limits = true;
        Ok(Self {
            t,
            kind,
            credentials,
            client,
        })
    }

    fn credential(&self, key: &str) -> Result<&str> {
        self.credentials[key]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("missing credential"))
    }

    async fn cf(
        &self,
        method: reqwest::Method,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Value> {
        let mut url = Url::parse(&format!("https://api.cloudflare.com/client/v4/{path}"))?;
        url.query_pairs_mut().extend_pairs(params.iter().copied());
        let mut request = self.client.request(method, url);
        if let Ok(token) = self.credential("CF_Token") {
            request = request.bearer_auth(token);
        } else {
            request = request
                .header("X-Auth-Key", self.credential("CF_Key")?)
                .header("X-Auth-Email", self.credential("CF_Email")?);
        }
        let (status, body, _) = response_json(self.t, request.send().await?).await?;
        ensure!(
            status.is_success() && body["success"] == true,
            "Cloudflare request failed"
        );
        Ok(body)
    }

    async fn cf_zone_info(&self, zone: &str) -> Result<Value> {
        let safe_id = |id: &str| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric());
        let zone_record = if let Ok(id) = self.credential("CF_Zone_ID") {
            ensure!(safe_id(id), "invalid zone ID");
            self.cf(reqwest::Method::GET, &format!("zones/{id}"), &[])
                .await?["result"]
                .clone()
        } else {
            let mut params = vec![("name", zone)];
            if let Ok(account) = self.credential("CF_Account_ID") {
                params.push(("account.id", account));
            }
            let body = self.cf(reqwest::Method::GET, "zones", &params).await?;
            let entries = body["result"]
                .as_array()
                .filter(|v| v.len() == 1)
                .ok_or_else(|| anyhow::anyhow!("zone not unique"))?;
            entries[0].clone()
        };
        ensure!(
            zone_record["name"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case(zone)),
            "zone changed"
        );
        ensure!(
            zone_record["id"].as_str().is_some_and(safe_id),
            "invalid zone ID"
        );
        Ok(zone_record)
    }

    async fn cf_zone(&self, zone: &str) -> Result<String> {
        Ok(self.cf_zone_info(zone).await?["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned())
    }

    pub(crate) async fn owns_delegation(
        &self,
        zone: &str,
        expected: &BTreeSet<String>,
    ) -> Result<bool> {
        if self.kind != "dns_cf" || expected.is_empty() {
            return Ok(false);
        }
        let info = self.cf_zone_info(zone).await?;
        let advertised: BTreeSet<_> = info["name_servers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|s| s.trim_end_matches('.').to_ascii_lowercase())
            .collect();
        Ok(info["status"] == "active" && &advertised == expected)
    }

    async fn ali(&self, params: Vec<(&str, String)>) -> Result<Value> {
        let body = alidns_request(
            self.t,
            &self.client,
            self.credential("Ali_Key")?,
            self.credential("Ali_Secret")?,
            params,
        )
        .await?;
        ensure!(body.get("Code").is_none(), "AliDNS request failed");
        Ok(body)
    }
    async fn dp(&self, action: &str, params: Vec<(&str, String)>) -> Result<Value> {
        let body = dnspod_request(
            self.t,
            &self.client,
            &format!("https://dnsapi.cn/{action}"),
            self.credential("DP_Id")?,
            self.credential("DP_Key")?,
            params,
        )
        .await?;
        if action == "Record.List"
            && body.pointer("/status/code").and_then(Value::as_str) == Some("10")
        {
            return Ok(json!({"records":[]}));
        }
        ensure!(
            body.pointer("/status/code").and_then(Value::as_str) == Some("1"),
            "DNSPod request failed"
        );
        Ok(body)
    }
    async fn tencent(&self, action: &str, params: Value) -> Result<Value> {
        tencentcloud_request(
            self.t,
            &self.client,
            self.credential("Tencent_SecretId")?,
            self.credential("Tencent_SecretKey")?,
            action,
            params,
        )
        .await
    }

    pub(crate) async fn list(&self, zone: &str, name: &str) -> Result<Vec<TxtRecord>> {
        let relative = if name == zone && self.kind == "dns_cf" {
            "@"
        } else {
            name.strip_suffix(&format!(".{zone}"))
                .ok_or_else(|| anyhow::anyhow!("outside zone"))?
        };
        let (body, path, keys, owner) = match self.kind {
            "dns_cf" => {
                let zone_id = self.cf_zone(zone).await?;
                let body = self
                    .cf(
                        reqwest::Method::GET,
                        &format!("zones/{zone_id}/dns_records"),
                        &[("name", name), ("type", "TXT"), ("per_page", "100")],
                    )
                    .await?;
                ensure!(
                    body.pointer("/result_info/total_pages")
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        <= 1,
                    "incomplete list"
                );
                (body, "/result", ["name", "type", "content", "id"], name)
            }
            "dns_ali" => {
                let body = self
                    .ali(vec![
                        ("Action", "DescribeSubDomainRecords".into()),
                        ("DomainName", zone.into()),
                        ("SubDomain", name.into()),
                        ("Type", "TXT".into()),
                        ("PageSize", "100".into()),
                    ])
                    .await?;
                ensure!(
                    body["TotalCount"].as_u64().unwrap_or(0) <= 100,
                    "incomplete list"
                );
                (
                    body,
                    "/DomainRecords/Record",
                    ["RR", "Type", "Value", "RecordId"],
                    relative,
                )
            }
            "dns_dp" => {
                let body = self
                    .dp(
                        "Record.List",
                        vec![
                            ("domain", zone.into()),
                            ("sub_domain", relative.into()),
                            ("record_type", "TXT".into()),
                            ("length", "100".into()),
                        ],
                    )
                    .await?;
                ensure!(
                    body.pointer("/info/record_total")
                        .and_then(Value::as_str)
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or(0)
                        <= 100,
                    "incomplete list"
                );
                (body, "/records", ["name", "type", "value", "id"], relative)
            }
            _ => {
                let body = match self
                    .tencent(
                        "DescribeRecordList",
                        json!({"Domain":zone,"Subdomain":relative,"RecordType":"TXT","Limit":100}),
                    )
                    .await
                {
                    Ok(body) => body,
                    Err(e)
                        if e.to_string()
                            .starts_with("ResourceNotFound.NoDataOfRecord:") =>
                    {
                        json!({"RecordList":[]})
                    }
                    Err(e) => return Err(e),
                };
                ensure!(
                    body.pointer("/RecordCountInfo/TotalCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        <= 100,
                    "incomplete list"
                );
                (
                    body,
                    "/RecordList",
                    ["Name", "Type", "Value", "RecordId"],
                    relative,
                )
            }
        };
        let entries = body
            .pointer(path)
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("incomplete provider response"))?;
        extract_records(entries, keys, owner, zone, name, self.kind == "dns_cf")
    }

    async fn missing_id(&self, record: &TxtRecord) -> Result<bool> {
        match self.kind {
            "dns_cf" => {
                let zone_id = self.cf_zone(&record.zone).await?;
                let path = format!("zones/{zone_id}/dns_records/{}", record.id);
                let mut request = self
                    .client
                    .get(format!("https://api.cloudflare.com/client/v4/{path}"));
                if let Ok(token) = self.credential("CF_Token") {
                    request = request.bearer_auth(token)
                } else {
                    request = request
                        .header("X-Auth-Key", self.credential("CF_Key")?)
                        .header("X-Auth-Email", self.credential("CF_Email")?)
                }
                let (status, body, _) = response_json(self.t, request.send().await?).await?;
                if body["errors"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|e| e["code"] == 81044))
                {
                    return Ok(true);
                }
                ensure!(
                    status.is_success() && body["success"] == true,
                    "Cloudflare record lookup failed"
                );
                Ok(false)
            }
            "dns_ali" => {
                let body = alidns_request(
                    self.t,
                    &self.client,
                    self.credential("Ali_Key")?,
                    self.credential("Ali_Secret")?,
                    vec![
                        ("Action", "DescribeDomainRecordInfo".into()),
                        ("RecordId", record.id.clone()),
                    ],
                )
                .await?;
                if body["Code"] == "InvalidRecordId.NotFound" {
                    return Ok(true);
                }
                ensure!(
                    body["RecordId"] == record.id && body.get("Code").is_none(),
                    "AliDNS record lookup failed"
                );
                Ok(false)
            }
            "dns_dp" => {
                let body = dnspod_request(
                    self.t,
                    &self.client,
                    "https://dnsapi.cn/Record.Info",
                    self.credential("DP_Id")?,
                    self.credential("DP_Key")?,
                    vec![
                        ("domain", record.zone.clone()),
                        ("record_id", record.id.clone()),
                    ],
                )
                .await?;
                match body.pointer("/status/code").and_then(Value::as_str) {
                    Some("8") => Ok(true),
                    Some("1") => Ok(false),
                    _ => anyhow::bail!("DNSPod record lookup failed"),
                }
            }
            _ => {
                match self
                    .tencent(
                        "DescribeRecord",
                        json!({"Domain":record.zone,"RecordId":record.id.parse::<u64>()?}),
                    )
                    .await
                {
                    Ok(_) => Ok(false),
                    Err(e)
                        if e.to_string()
                            .starts_with("ResourceNotFound.NoDataOfRecord:")
                            || e.to_string()
                                .starts_with("InvalidParameter.RecordIdInvalid:") =>
                    {
                        Ok(true)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    pub(crate) async fn delete_selected(
        &self,
        record: &TxtRecord,
        still_observed: bool,
    ) -> Result<&'static str> {
        ensure!(
            !record.id.is_empty() && record.id.bytes().all(|b| b.is_ascii_alphanumeric()),
            "invalid record ID"
        );
        let current = self.list(&record.zone, &record.name).await?;
        if still_observed
            && current.contains(record)
            && current.iter().filter(|r| r.value == record.value).count() == 1
        {
            self.delete(record).await?;
            return Ok("deleted");
        }
        if current
            .iter()
            .any(|r| r.id == record.id || r.value == record.value)
            || !self.missing_id(record).await?
        {
            return Ok("changed");
        }
        Ok("absent")
    }

    async fn delete(&self, record: &TxtRecord) -> Result<()> {
        ensure!(
            !record.id.is_empty() && record.id.bytes().all(|b| b.is_ascii_alphanumeric()),
            "invalid record ID"
        );
        match self.kind {
            "dns_cf" => {
                let zone_id = self.cf_zone(&record.zone).await?;
                self.cf(
                    reqwest::Method::DELETE,
                    &format!("zones/{zone_id}/dns_records/{}", record.id),
                    &[],
                )
                .await?;
            }
            "dns_dp" => {
                self.dp(
                    "Record.Remove",
                    vec![
                        ("domain", record.zone.clone()),
                        ("record_id", record.id.clone()),
                    ],
                )
                .await?;
            }
            "dns_ali" => {
                self.ali(vec![
                    ("Action", "DeleteDomainRecord".into()),
                    ("RecordId", record.id.clone()),
                ])
                .await?;
            }
            _ => {
                self.tencent(
                    "DeleteRecord",
                    json!({"Domain":record.zone,"RecordId":record.id.parse::<u64>()?}),
                )
                .await?;
            }
        }
        Ok(())
    }
}

fn extract_records(
    entries: &[Value],
    keys: [&str; 4],
    owner: &str,
    zone: &str,
    name: &str,
    quoted: bool,
) -> Result<Vec<TxtRecord>> {
    let mut records = Vec::new();
    for row in entries {
        if row[keys[0]]
            .as_str()
            .is_none_or(|v| !v.trim_end_matches('.').eq_ignore_ascii_case(owner))
            || row[keys[1]] != "TXT"
        {
            continue;
        }
        let raw = row[keys[2]]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing TXT value"))?;
        // The provider JSON has already been decoded. TXT presentation is
        // DNS syntax, not a second JSON string: interpreting e.g. a literal
        // backslash-u escape here could select a different TXT value.
        let literal = if quoted {
            let unwrapped = if let Some(inner) = raw.strip_prefix('"') {
                inner
                    .strip_suffix('"')
                    .ok_or_else(|| anyhow::anyhow!("invalid TXT presentation"))?
            } else {
                raw
            };
            ensure!(
                !unwrapped.contains(['\\', '"']),
                "ambiguous TXT presentation"
            );
            unwrapped
        } else {
            raw
        };
        let value = literal.to_owned();
        let id = match &row[keys[3]] {
            Value::String(id) if !id.is_empty() => id.clone(),
            Value::Number(id) => id.to_string(),
            _ => anyhow::bail!("missing record ID"),
        };
        records.push(TxtRecord {
            id,
            zone: zone.into(),
            name: name.into(),
            value,
        });
    }
    records.sort();
    Ok(records)
}

/// Select from an actual hook list response, which can include fuzzy matches
/// for another owner, case-insensitive content matches, or other record types.
pub(crate) fn native_cleanup_id(
    provider: &str,
    zone: &str,
    name: &str,
    value: &str,
    body: &Value,
) -> Result<Option<String>> {
    let relative = if name == zone {
        "@"
    } else {
        name.strip_suffix(&format!(".{zone}"))
            .ok_or_else(|| anyhow::anyhow!("outside DNS zone"))?
    };
    ensure!(
        name.starts_with("_acme-challenge.") && !value.is_empty(),
        "invalid challenge"
    );
    let (path, keys, owner, total) = match provider {
        "dns_cf" if body["success"] == true => (
            "/result",
            ["name", "type", "content", "id"],
            name,
            body.pointer("/result_info/total_count"),
        ),
        "dns_dp" if body.pointer("/status/code") == Some(&json!("1")) => (
            "/records",
            ["name", "type", "value", "id"],
            relative,
            body.pointer("/info/record_total"),
        ),
        "dns_ali" if body.get("Code").is_none() => (
            "/DomainRecords/Record",
            ["RR", "Type", "Value", "RecordId"],
            relative,
            body.get("TotalCount"),
        ),
        "dns_tencent" if body.pointer("/Response/Error").is_none() => (
            "/Response/RecordList",
            ["Name", "Type", "Value", "RecordId"],
            relative,
            body.pointer("/Response/RecordCountInfo/TotalCount"),
        ),
        _ => anyhow::bail!("unrecognized provider response"),
    };
    let entries = body
        .pointer(path)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("missing record list"))?;
    ensure!(
        body.pointer("/result_info/total_pages")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            <= 1,
        "incomplete record list"
    );
    if let Some(total) = total {
        let total = total.as_u64().or_else(|| total.as_str()?.parse().ok());
        ensure!(
            total == Some(entries.len() as u64),
            "incomplete record list"
        );
    }
    let mut records = extract_records(entries, keys, owner, zone, name, provider == "dns_cf")?
        .into_iter()
        .filter(|r| r.value == value);
    let first = records.next();
    ensure!(records.next().is_none(), "ambiguous TXT value");
    if let Some(record) = first {
        ensure!(
            record.id.bytes().all(|b| b.is_ascii_alphanumeric()),
            "invalid record ID"
        );
        Ok(Some(record.id))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
