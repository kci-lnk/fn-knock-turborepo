use super::*;
mod journal;
pub(super) use journal::{
    JobRates, apply_job_rates, flush_interrupted_rates, restore_interrupted_limit,
};

pub(super) fn analyze_acme_logs(job: &Value, logs: &[Value], t: &Translator) -> Value {
    let logs = logs
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if logs.is_empty() || job["status"] == "succeeded" {
        return Value::Null;
    }

    let provider = job
        .get("provider")
        .and_then(Value::as_str)
        .map(str::to_string);
    let has = |needle: &str| logs.iter().any(|line| contains_ci(line, needle));
    let has_code = |code: &str| {
        logs.iter()
            .any(|line| contains_ci(line, "\"code\"") && line.contains(code))
    };
    let is_cloudflare =
        provider.as_deref() == Some("dns_cf") || has("Cloudflare") || has("X-Auth-Key");

    if is_cloudflare {
        if has("Invalid format for X-Auth-Key header") || has_code("6103") {
            return analysis_object(
                "dns_credentials_invalid",
                Some("dns_cf"),
                t.t("server.acmeRoutes.cloudflareInvalidKey"),
                pick_evidence(&logs, |line| {
                    contains_ci(line, "X-Auth-Key")
                        || (contains_ci(line, "\"code\"") && line.contains("6103"))
                }),
            );
        }

        if has("Invalid format for X-Auth-Email header") {
            return analysis_object(
                "dns_credentials_invalid_email",
                Some("dns_cf"),
                t.t("server.acmeRoutes.cloudflareInvalidEmail"),
                pick_evidence(&logs, |line| contains_ci(line, "X-Auth-Email")),
            );
        }

        if has("Invalid request headers") || has_code("6003") {
            return analysis_object(
                "dns_credentials_invalid",
                Some("dns_cf"),
                t.t("server.acmeRoutes.cloudflareInvalidHeaders"),
                pick_evidence(&logs, |line| {
                    contains_ci(line, "Invalid request headers")
                        || (contains_ci(line, "\"code\"") && line.contains("6003"))
                }),
            );
        }
    }

    if let Some((limit, _)) = detect_limit(&logs, time_utils::now_ms() / 1000) {
        let (reason, key) = match limit.source.as_str() {
            "ca" => ("acme_frequency_limited", "caRateLimited"),
            "dns" => ("dns_api_rate_limited", "dnsApiRateLimited"),
            _ => ("unknown_rate_limited", "unknownRateLimited"),
        };
        return analysis_object(
            reason,
            provider.as_deref(),
            acme_route_text(t, key),
            pick_evidence(&logs, |l| {
                limit_signal(l) || parse_retry_after_seconds(l).is_some()
            }),
        );
    }

    if logs
        .iter()
        .any(|line| contains_ci(line, "failed") || contains_ci(line, "invalid"))
    {
        return analysis_object(
            "unknown",
            provider.as_deref(),
            t.t("server.acmeRoutes.logUnknownFailure"),
            pick_evidence(&logs, |line| {
                contains_ci(line, "failed") || contains_ci(line, "invalid")
            }),
        );
    }

    Value::Null
}

fn analysis_object(
    reason: &str,
    provider: Option<&str>,
    message: String,
    evidence: Option<Vec<String>>,
) -> Value {
    let mut object = Map::new();
    object.insert("reason".to_string(), json!(reason));
    if let Some(provider) = provider {
        object.insert("provider".to_string(), json!(provider));
    }
    object.insert("message".to_string(), json!(message));
    if let Some(evidence) = evidence {
        object.insert("evidence".to_string(), json!(evidence));
    }
    Value::Object(object)
}

fn pick_evidence(logs: &[String], matches: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    let mut hits = Vec::new();
    for line in logs.iter().rev() {
        if line.is_empty() || !matches(line) {
            continue;
        }
        hits.push(line.clone());
        if hits.len() >= 3 {
            break;
        }
    }
    if hits.is_empty() {
        None
    } else {
        hits.reverse();
        Some(hits)
    }
}

fn parse_retry_after_seconds(line: &str) -> Option<i64> {
    let lower = line.to_ascii_lowercase();
    let (_, tail) = lower.split_once("retryafter")?;
    let tail = tail.trim_start();
    let tail = tail.strip_prefix('=')?.trim_start();
    let digits = tail
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<i64>().ok()
    }
}

fn contains_ci(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

use super::preflight::Cooldown;

fn limit_signal(line: &str) -> bool {
    let s = line.to_ascii_lowercase();
    s.contains("urn:ietf:params:acme:error:ratelimited")
        || s.contains("too many requests")
        || s.contains("rate limit exceeded")
        || s.contains("rate-limited")
        || http_429(line)
}

fn quota_detail(line: &str) -> bool {
    let s = line.to_ascii_lowercase();
    [
        "too many new orders",
        "too many certificates",
        "too many failed authorizations",
        "failed validation",
        "exact set of identifiers",
        "per account",
        "per token",
        "per credential",
        "per api token",
    ]
    .iter()
    .any(|text| s.contains(text))
}

fn http_429(line: &str) -> bool {
    static STATUS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
        r#"(?i)\b(?:http(?:/\d+(?:\.\d+)?)?|_?code|status(?:[_ -]?code)?|response(?:[_ -]?code)?|error)[\s:"'=\[(]+429\b"#
    ).expect("static HTTP status pattern")
    });
    STATUS.is_match(line)
}

fn retry_time(line: &str, now: i64) -> Option<i64> {
    let lower = line.to_ascii_lowercase();
    for marker in ["retry-after", "retryafter", "retry after"] {
        if let Some((_, tail)) = lower.split_once(marker) {
            let raw = &line[line.len() - tail.len()..];
            let raw = raw.trim_start_matches([' ', ':', '=', '\'', '"']);
            if let Some(date) = raw
                .get(..29)
                .and_then(|s| httpdate::parse_http_date(s).ok())
            {
                return i64::try_from(date.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
                    .ok();
            }
            let stamp = raw
                .split(['"', '\'', ',', ';', ')', ' '])
                .next()
                .unwrap_or_default();
            if let Ok(ts) = OffsetDateTime::parse(stamp, &Rfc3339) {
                return Some(ts.unix_timestamp());
            }
            if let Some(date) = raw.get(..19)
                && raw
                    .get(19..)
                    .is_some_and(|suffix| suffix.starts_with(" UTC") || suffix.starts_with(" GMT"))
                && let Ok(ts) =
                    OffsetDateTime::parse(&format!("{}Z", date.replace(' ', "T")), &Rfc3339)
            {
                return Some(ts.unix_timestamp());
            }
            let digits = raw.split(|c: char| !c.is_ascii_digit()).next()?;
            if raw[digits.len()..].starts_with(['-', ':', 'T']) {
                continue;
            }
            if let Ok(seconds) = digits.parse::<i64>() {
                return now.checked_add(seconds);
            }
        }
    }
    None
}

pub(super) fn detect_limit(lines: &[String], now: i64) -> Option<(Cooldown, String)> {
    let hit = lines.iter().rposition(|l| {
        limit_signal(l)
            || (parse_retry_after_seconds(l).is_some() && contains_ci(l, "will not retry"))
    })?;
    let line = lines[hit].to_ascii_lowercase();
    let start = hit.saturating_sub(12);
    let context = &lines[start..=(hit + 5).min(lines.len() - 1)];
    let endpoint = lines[..=hit]
        .iter()
        .rev()
        .find_map(|line| {
            let start = line.find("https://")?;
            let url = line[start..].split([' ', '\'', '"', '\t']).next()?;
            url::Url::parse(url)
                .ok()?
                .host_str()
                .map(str::to_ascii_lowercase)
        })
        .unwrap_or_default();
    let source = if context
        .iter()
        .any(|l| contains_ci(l, "urn:ietf:params:acme:error:ratelimited"))
        || endpoint.ends_with(".letsencrypt.org")
        || endpoint == "acme.zerossl.com"
    {
        "ca"
    } else if [
        "api.cloudflare.com",
        "alidns.aliyuncs.com",
        "dnsapi.cn",
        "dnspod.tencentcloudapi.com",
    ]
    .iter()
    .any(|v| endpoint == *v)
        || ["dns api", "dns provider", "cloudflare api"]
            .iter()
            .any(|v| line.contains(v))
    {
        "dns"
    } else {
        "unknown"
    };
    let until = context.iter().filter_map(|l| retry_time(l, now)).max();
    Some((
        Cooldown {
            source: source.into(),
            retry_at: until
                .unwrap_or_else(|| now + super::tasks::acme_renew_failure_backoff_seconds()),
            estimated: until.is_none(),
        },
        context
            .iter()
            .filter(|l| limit_signal(l) || quota_detail(l))
            .map(|l| l.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

fn cooldown_key(scope: &str, source: &str) -> String {
    format!(
        "fn_knock:acme:limit:{source}:{}",
        hex::encode(Sha256::digest(scope))
    )
}
pub(super) async fn issuance_ca(state: &AppState) -> anyhow::Result<String> {
    if crate::runtime_profile::deployment_target(state) == "windows" {
        return Ok("letsencrypt".into());
    }
    Ok(ensure_client_settings(state).await?["certificateAuthority"]
        .as_str()
        .unwrap_or(DEFAULT_ACME_CERTIFICATE_AUTHORITY)
        .to_owned())
}

fn scope_keys(app: &Value, ca: &str) -> Vec<String> {
    let kind = app["dnsType"].as_str().unwrap_or_default();
    let credential_fields: &[&str] = match kind {
        "dns_cf"
            if app["credentials"]["CF_Token"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty()) =>
        {
            &["CF_Token"]
        }
        "dns_cf" => &["CF_Key", "CF_Email"],
        "dns_dp" => &["DP_Id", "DP_Key"],
        "dns_ali" => &["Ali_Key", "Ali_Secret"],
        "dns_tencent" => &["Tencent_SecretId", "Tencent_SecretKey"],
        _ => &[],
    };
    let credentials = if credential_fields.is_empty() {
        app["credentials"].clone()
    } else {
        json!(
            credential_fields
                .iter()
                .map(|key| &app["credentials"][*key])
                .collect::<Vec<_>>()
        )
    };
    let mut keys = vec![
        format!("application:{}", app["id"]),
        format!("account:{ca}"),
        format!("credential:{kind}:{credentials}"),
    ];
    let mut domains = BTreeSet::new();
    let mut zones = BTreeSet::new();
    if let Some(sans) = app["domains"].as_array() {
        for domain in sans.iter().filter_map(Value::as_str) {
            let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
            domains.insert(domain.clone());
            let mut suffix = domain.trim_start_matches("*.");
            while let Some((_, parent)) = suffix.split_once('.') {
                zones.insert(suffix.to_string());
                suffix = parent;
            }
            keys.push(format!(
                "identifier:{ca}:{}",
                domain.trim_start_matches("*.")
            ));
        }
    }
    keys.extend(zones.into_iter().map(|name| format!("zone:{ca}:{name}")));
    let exact = domains.into_iter().collect::<Vec<_>>().join(",");
    keys.push(format!("san-set:{ca}:{exact}"));
    keys.push(format!("request:{exact}"));
    keys.push(format!("dns-request:{kind}:{exact}"));
    keys.push(format!("registration:{ca}"));
    keys
}
pub(super) async fn active_cooldown(
    state: &AppState,
    app: &Value,
) -> anyhow::Result<Option<Cooldown>> {
    cooldown_for_ca(state, app, &issuance_ca(state).await?).await
}

pub(super) async fn cooldown_for_ca(
    state: &AppState,
    app: &Value,
    ca: &str,
) -> anyhow::Result<Option<Cooldown>> {
    matching_cooldown(state, &scope_keys(app, ca), &["ca", "dns", "unknown"]).await
}

pub(super) async fn registration_cooldown(
    state: &AppState,
    ca: &str,
) -> anyhow::Result<Option<Cooldown>> {
    matching_cooldown(
        state,
        &[format!("registration:{ca}"), format!("account:{ca}")],
        &["ca"],
    )
    .await
}
pub(super) async fn persist_registration_limit(
    state: &AppState,
    ca: &str,
    limit: &Cooldown,
) -> anyhow::Result<()> {
    store_limit(
        state,
        &[cooldown_key(&format!("registration:{ca}"), "ca")],
        limit,
    )
    .await
}

pub(super) async fn dns_cooling(state: &AppState, app: &Value) -> anyhow::Result<bool> {
    Ok(matching_cooldown(state, &scope_keys(app, ""), &["dns"])
        .await?
        .is_some())
}

async fn matching_cooldown(
    state: &AppState,
    scopes: &[String],
    sources: &[&str],
) -> anyhow::Result<Option<Cooldown>> {
    let mut keys = scopes
        .iter()
        .flat_map(|scope| sources.iter().map(|source| cooldown_key(scope, source)))
        .collect::<Vec<_>>();
    keys.push(journal::PENDING_RATES.into());
    let mut raw = state.storage.store.mget_string_values(&keys).await?;
    let pending = raw
        .pop()
        .flatten()
        .map(|s| serde_json::from_str::<Value>(&s))
        .transpose()?;
    let mut limits = raw
        .into_iter()
        .flatten()
        .map(|s| serde_json::from_str::<Cooldown>(&s))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(values) = pending.as_ref().and_then(|v| v["observations"].as_object()) {
        for key in &keys[..keys.len() - 1] {
            if let Some(value) = values.get(key) {
                limits.push(serde_json::from_value(value.clone())?);
            }
        }
    }
    let now = time_utils::now_ms() / 1000;
    let latest = limits
        .into_iter()
        .filter(|l| l.retry_at > now)
        .max_by_key(|l| l.retry_at);
    Ok(latest)
}
pub(super) async fn record_cooldown(
    state: &AppState,
    app: &Value,
    logs: &[Value],
) -> anyhow::Result<()> {
    record_cooldown_for_ca(state, app, &issuance_ca(state).await?, logs)
        .await
        .map(|_| ())
}

pub(super) async fn record_cooldown_for_ca(
    state: &AppState,
    app: &Value,
    ca: &str,
    logs: &[Value],
) -> anyhow::Result<Option<Cooldown>> {
    let now = time_utils::now_ms() / 1000;
    let lines: Vec<_> = logs
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let Some((limit, line)) = detect_limit(&lines, now) else {
        return Ok(None);
    };
    let keys = limit_keys(&scope_keys(app, ca), &limit, &line);
    store_limit(state, &keys, &limit).await?;
    Ok(Some(limit))
}
fn limit_keys(scopes: &[String], limit: &Cooldown, line: &str) -> Vec<String> {
    // A fresh application ID with the same SANs is still the same request.
    let mut selected = vec![&scopes[0]];
    let request_prefix = match limit.source.as_str() {
        "ca" => "san-set:",
        "dns" => "dns-request:",
        _ => "request:",
    };
    if let Some(exact) = scopes
        .iter()
        .find(|scope| scope.starts_with(request_prefix))
    {
        selected.push(exact);
    }

    if limit.source == "ca"
        && (line.contains("per account") || line.contains("this account"))
        && line.contains("orders")
    {
        selected.push(&scopes[1])
    }
    if limit.source == "dns"
        && ["account", "token", "credential"]
            .iter()
            .any(|v| line.contains(v))
    {
        selected.push(&scopes[2])
    }
    if limit.source == "ca" {
        let mentions_domain = |scope: &str| {
            scope.rsplit(':').next().is_some_and(|domain| {
                line.split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '.')
                    .any(|token| token.trim_end_matches('.') == domain)
            })
        };
        for scope in &scopes[3..] {
            if (scope.starts_with("identifier:")
                && (line.contains("authorization") || line.contains("failed validation"))
                && mentions_domain(scope))
                || (scope.starts_with("zone:")
                    && line.contains("certificates")
                    && !line.contains("exact set")
                    && mentions_domain(scope))
                || (scope.starts_with("san-set:") && line.contains("exact set of identifiers"))
            {
                selected.push(scope);
            }
        }
    }
    selected
        .into_iter()
        .map(|scope| cooldown_key(scope, &limit.source))
        .collect()
}

async fn store_limit(state: &AppState, keys: &[String], limit: &Cooldown) -> anyhow::Result<()> {
    state
        .storage
        .store
        .extend_acme_cooldowns(
            keys,
            &serde_json::to_value(limit)?,
            limit.retry_at,
            time_utils::now_ms() / 1000,
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod limit_tests {
    use super::*;
    fn detect(lines: &[&str]) -> Option<Cooldown> {
        detect_limit(&lines.iter().map(|v| v.to_string()).collect::<Vec<_>>(), 0).map(|v| v.0)
    }
    #[test]
    fn retry_after_without_failure_does_not_start_a_cooldown() {
        assert!(detect(&["HTTP/1.1 200 OK", "Retry-After: 600", "processing"]).is_none());
        assert!(
            detect(&[
                "HTTP 200 success, record ID 429",
                "certificate error: serial 429",
                "challenge abc429def"
            ])
            .is_none()
        );
        assert!(detect(&["code='429'"]).is_some());
    }
    #[test]
    fn identifies_ca_dns_and_unknown_sources_and_retry_dates() {
        let dns = detect(&[
            "url='https://api.cloudflare.com/client/v4/zones'",
            "HTTP/2 429",
            "Retry-After: 25",
        ])
        .unwrap();
        assert_eq!(
            (dns.source.as_str(), dns.retry_at, dns.estimated),
            ("dns", 25, false)
        );
        let ca = detect(&[
            "urn:ietf:params:acme:error:rateLimited",
            "Retry-After: Wed, 21 Oct 2015 07:28:00 GMT",
        ])
        .unwrap();
        assert_eq!(
            (ca.source.as_str(), ca.retry_at, ca.estimated),
            ("ca", 1445412480, false)
        );
        let unknown = detect(&["HTTP 429 Too Many Requests"]).unwrap();
        assert_eq!(unknown.source, "unknown");
        assert!(unknown.estimated);
        assert_eq!(
            retry_time("retry after 2015-10-21 07:28:00 UTC", 0),
            Some(1445412480)
        );
        assert_eq!(
            retry_time("retry after 2015-10-21T07:28:00.000-08:00", 0),
            Some(1445441280)
        );
    }
}
