use super::*;

pub(super) fn analyze_acme_logs(job: &Value, logs: &[Value], t: &Translator) -> Value {
    let logs = logs
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if logs.is_empty() {
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

    if let Some((retry_line, seconds)) = logs
        .iter()
        .rev()
        .find_map(|line| parse_retry_after_seconds(line).map(|seconds| (line, seconds)))
        && (contains_ci(retry_line, "will not retry") || contains_ci(retry_line, "too large"))
        && seconds > 600
    {
        return analysis_object(
            "acme_frequency_limited",
            provider.as_deref(),
            t.t_params(
                "server.acmeRoutes.acmeFrequencyLimited",
                &[("seconds", seconds.to_string())],
            ),
            pick_evidence(&logs, |line| {
                parse_retry_after_seconds(line).is_some()
                    || contains_ci(line, "will not retry")
                    || contains_ci(line, "too large")
            }),
        );
    }

    if logs.iter().any(|line| {
        contains_ci(line, "rate limit")
            || contains_ci(line, "too many requests")
            || line.contains("429")
    }) {
        return analysis_object(
            "dns_api_rate_limited",
            provider.as_deref(),
            t.t("server.acmeRoutes.dnsApiRateLimited"),
            pick_evidence(&logs, |line| {
                contains_ci(line, "rate limit")
                    || contains_ci(line, "too many requests")
                    || line.contains("429")
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
