use super::*;

pub(super) use crate::time_utils::{
    node_iso_after_seconds as iso_after_seconds_node, node_iso_now as now_node_iso,
};

pub(super) fn normalize_acme_application(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let id = non_empty_string(raw.get("id"))?;
    let domains = raw
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let primary_domain = raw
        .get("primaryDomain")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .or_else(|| domains.first().cloned())?;
    let dns_type = non_empty_string(raw.get("dnsType"))?;
    let created_at = raw
        .get("createdAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)?;
    let updated_at = raw
        .get("updatedAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)
        .unwrap_or_else(|| created_at.clone());
    let mut object = Map::new();
    object.insert("id".to_string(), json!(id));
    insert_optional_string(&mut object, "name", raw.get("name"));
    object.insert("domains".to_string(), json!(domains));
    object.insert("primaryDomain".to_string(), json!(primary_domain));
    object.insert("dnsType".to_string(), json!(dns_type));
    object.insert(
        "credentials".to_string(),
        normalize_string_record(raw.get("credentials")),
    );
    object.insert(
        "renewEnabled".to_string(),
        json!(raw.get("renewEnabled").and_then(Value::as_bool) != Some(false)),
    );
    object.insert("createdAt".to_string(), json!(created_at));
    object.insert("updatedAt".to_string(), json!(updated_at));
    insert_optional_string(&mut object, "latestJobId", raw.get("latestJobId"));
    insert_optional_value(
        &mut object,
        "latestJobStatus",
        normalize_latest_job_status(raw.get("latestJobStatus")),
    );
    insert_optional_value(
        &mut object,
        "latestJobTrigger",
        normalize_job_trigger(raw.get("latestJobTrigger")),
    );
    insert_optional_string(&mut object, "latestJobAt", raw.get("latestJobAt"));
    insert_optional_string(&mut object, "lastError", raw.get("lastError"));
    Some(Value::Object(object))
}

pub(super) fn normalize_issued_certificate(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let application_id = non_empty_string(raw.get("applicationId"))?;
    let primary_domain = raw
        .get("primaryDomain")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())?;
    let cert = non_empty_string(raw.get("cert"))?;
    let key = non_empty_string(raw.get("key"))?;
    let created_at = raw
        .get("createdAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)?;
    let updated_at = raw
        .get("updatedAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)
        .unwrap_or_else(|| created_at.clone());
    let cert_info = normalize_cert_info(raw.get("certInfo"))?;
    let mut object = Map::new();
    object.insert("applicationId".to_string(), json!(application_id));
    object.insert("primaryDomain".to_string(), json!(primary_domain));
    object.insert("cert".to_string(), json!(cert));
    object.insert("key".to_string(), json!(key));
    object.insert("certInfo".to_string(), cert_info);
    object.insert("createdAt".to_string(), json!(created_at));
    object.insert("updatedAt".to_string(), json!(updated_at));
    insert_optional_string(
        &mut object,
        "libraryCertificateId",
        raw.get("libraryCertificateId"),
    );
    insert_optional_string(&mut object, "libraryLinkedAt", raw.get("libraryLinkedAt"));
    Some(Value::Object(object))
}

pub(super) fn normalize_acme_job(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let id = non_empty_string(raw.get("id"))?;
    let domains = raw
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let created_at = raw
        .get("createdAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)?;
    let status = normalize_job_status(raw.get("status"))?;
    if domains.is_empty() {
        return None;
    }
    let method = match raw.get("method").and_then(Value::as_str) {
        Some("http") => "http",
        Some("https") => "https",
        _ => "dns",
    };
    let mut object = Map::new();
    object.insert("id".to_string(), json!(id));
    insert_optional_string(&mut object, "applicationId", raw.get("applicationId"));
    object.insert("domains".to_string(), json!(domains));
    object.insert("method".to_string(), json!(method));
    insert_optional_string(&mut object, "provider", raw.get("provider"));
    insert_optional_value(
        &mut object,
        "trigger",
        normalize_job_trigger(raw.get("trigger")),
    );
    object.insert("createdAt".to_string(), json!(created_at));
    insert_optional_string(&mut object, "startedAt", raw.get("startedAt"));
    insert_optional_string(&mut object, "finishedAt", raw.get("finishedAt"));
    object.insert("status".to_string(), json!(status));
    object.insert(
        "progress".to_string(),
        json!(
            raw.get("progress")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .clamp(0, 100)
        ),
    );
    insert_optional_string(&mut object, "message", raw.get("message"));
    Some(Value::Object(object))
}

pub(super) fn normalize_client_settings(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let ca = match raw.get("certificateAuthority").and_then(Value::as_str) {
        Some("letsencrypt") => "letsencrypt",
        _ => DEFAULT_ACME_CERTIFICATE_AUTHORITY,
    };
    Some(json!({
        "certificateAuthority": ca,
        "updatedAt": raw
            .get("updatedAt")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(now_node_iso),
    }))
}

pub(super) fn normalize_cert_info(value: Option<&Value>) -> Option<Value> {
    let raw = value?.as_object()?;
    let issuer = non_empty_string(raw.get("issuer"))?;
    let subject = non_empty_string(raw.get("subject"))?;
    let valid_from = non_empty_string(raw.get("validFrom"))?;
    let valid_to = non_empty_string(raw.get("validTo"))?;
    let serial_number = non_empty_string(raw.get("serialNumber"))?;
    let dns_names = raw
        .get("dnsNames")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    Some(json!({
        "issuer": issuer,
        "subject": subject,
        "validFrom": valid_from,
        "validTo": valid_to,
        "dnsNames": dns_names,
        "serialNumber": serial_number,
    }))
}

pub(super) fn issued_certificate_compatible(application: &Value, certificate: &Value) -> bool {
    if certificate.get("primaryDomain").and_then(Value::as_str)
        != application.get("primaryDomain").and_then(Value::as_str)
    {
        return false;
    }
    let app_domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let cert_domains = certificate
        .pointer("/certInfo/dnsNames")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    normalized_domain_signature(&app_domains) == normalized_domain_signature(&cert_domains)
}

pub(super) fn normalize_domain_list<'a>(values: impl Iterator<Item = &'a Value>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut domains = Vec::new();
    for value in values {
        let Some(domain) = value
            .as_str()
            .map(|value| value.trim().to_ascii_lowercase())
        else {
            continue;
        };
        if domain.is_empty() || !seen.insert(domain.clone()) {
            continue;
        }
        domains.push(domain);
    }
    domains
}

pub(super) fn normalized_domain_signature(domains: &[String]) -> String {
    let mut normalized = domains
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized.join("\n")
}

pub(super) fn normalize_string_record(value: Option<&Value>) -> Value {
    let mut output = Map::new();
    let Some(object) = value.and_then(Value::as_object) else {
        return Value::Object(output);
    };
    for (key, value) in object {
        let key = key.trim();
        let value = value.as_str().unwrap_or("").trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        output.insert(key.to_string(), Value::String(value.to_string()));
    }
    Value::Object(output)
}

pub(super) fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn normalize_timestamp(value: &str) -> Option<String> {
    time_utils::normalize_node_iso(value)
}

pub(super) fn insert_optional_string(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<&Value>,
) {
    if let Some(value) = non_empty_string(value) {
        object.insert(key.to_string(), Value::String(value));
    }
}

pub(super) fn insert_optional_value(object: &mut Map<String, Value>, key: &str, value: Value) {
    if !value.is_null() {
        object.insert(key.to_string(), value);
    }
}

pub(super) fn normalize_job_status(value: Option<&Value>) -> Option<&'static str> {
    match value.and_then(Value::as_str) {
        Some("queued") => Some("queued"),
        Some("running") => Some("running"),
        Some("succeeded") => Some("succeeded"),
        Some("failed") => Some("failed"),
        Some("stopped") => Some("stopped"),
        _ => None,
    }
}

pub(super) fn normalize_latest_job_status(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_str) {
        Some("idle") => json!("idle"),
        Some("queued") => json!("queued"),
        Some("running") => json!("running"),
        Some("succeeded") => json!("succeeded"),
        Some("failed") => json!("failed"),
        Some("stopped") => json!("stopped"),
        _ => Value::Null,
    }
}

pub(super) fn normalize_job_trigger(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_str) {
        Some("manual_request") => json!("manual_request"),
        Some("auto_renew") => json!("auto_renew"),
        _ => Value::Null,
    }
}

pub(super) fn normalize_log_limit(value: Option<&str>) -> usize {
    let parsed = value
        .map(parse_js_number_like_query)
        .unwrap_or(Some(DEFAULT_ACME_LOG_LIMIT as f64))
        .unwrap_or(DEFAULT_ACME_LOG_LIMIT as f64);
    if parsed.is_nan() {
        return DEFAULT_ACME_LOG_LIMIT;
    }
    let clamped = parsed.max(1.0).min(MAX_ACME_LOG_LIMIT as f64);
    if !clamped.is_finite() {
        return if clamped.is_sign_positive() {
            MAX_ACME_LOG_LIMIT
        } else {
            1
        };
    }
    clamped.floor() as usize
}

pub(super) fn parse_js_number_like_query(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }
    trimmed.parse::<f64>().ok()
}

pub(super) fn build_application_id(seed: Option<&str>) -> String {
    let normalized_seed = seed.unwrap_or("").trim().to_ascii_lowercase();
    if !normalized_seed.is_empty() {
        let mut hasher = Sha256::new();
        hasher.update(normalized_seed.as_bytes());
        let digest = hex::encode(hasher.finalize());
        return format!("acme_app_{}", &digest[..16]);
    }
    format!("acme_app_{}", uuid::Uuid::new_v4().simple())
}

pub(super) fn acme_certificate_archive_stem(domain: &str) -> String {
    let trimmed = domain.trim().trim_end_matches('.');
    let wildcard_safe = trimmed
        .strip_prefix("*.")
        .map(|suffix| format!("wildcard.{suffix}"))
        .unwrap_or_else(|| trimmed.to_string());
    let portable = wildcard_safe
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let portable = portable.trim_matches([' ', '.']);
    if portable.is_empty() {
        "certificate".to_string()
    } else {
        portable.to_string()
    }
}

pub(super) fn zip_acme_cert_pair(domain: &str, cert: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::<u8>::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let archive_stem = acme_certificate_archive_stem(domain);
    zip.start_file(format!("{archive_stem}.cert.pem"), options)?;
    zip.write_all(cert.as_bytes())?;
    zip.start_file(format!("{archive_stem}.key.pem"), options)?;
    zip.write_all(key.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}
