use super::*;

pub(super) fn normalize_ssl_config(value: Option<&Value>) -> Value {
    let raw = value.cloned().unwrap_or_else(|| json!({}));
    let mut certificates = raw
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(normalize_managed_certificate)
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    certificates.retain(|item| {
        let id = item.get("id").and_then(Value::as_str).unwrap_or("");
        !id.is_empty() && seen.insert(id.to_string())
    });
    let legacy_cert = raw.get("cert").and_then(Value::as_str).unwrap_or("").trim();
    let legacy_key = raw.get("key").and_then(Value::as_str).unwrap_or("").trim();
    let mut legacy_match_id = String::new();
    if !legacy_cert.is_empty() && !legacy_key.is_empty() {
        legacy_match_id = certificates
            .iter()
            .find(|item| {
                item.get("cert").and_then(Value::as_str) == Some(legacy_cert)
                    && item.get("key").and_then(Value::as_str) == Some(legacy_key)
            })
            .and_then(|item| item.get("id").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();
        if legacy_match_id.is_empty() {
            if let Some(migrated) = normalize_managed_certificate(json!({
                "id": build_ssl_certificate_id(legacy_cert, legacy_key),
                "label": default_certificate_label("current", None),
                "source": "manual",
                "cert": legacy_cert,
                "key": legacy_key
            })) {
                legacy_match_id = migrated
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                certificates.insert(0, migrated);
            }
        }
    }
    let active_id = raw
        .get("active_cert_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| {
            certificates
                .iter()
                .any(|item| item.get("id").and_then(Value::as_str) == Some(*id))
        })
        .unwrap_or(&legacy_match_id)
        .to_string();
    let active = certificates
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(active_id.as_str()));
    json!({
        "cert": active.and_then(|item| item.get("cert").and_then(Value::as_str)).unwrap_or(""),
        "key": active.and_then(|item| item.get("key").and_then(Value::as_str)).unwrap_or(""),
        "active_cert_id": active.and_then(|item| item.get("id").and_then(Value::as_str)).unwrap_or(""),
        "deployment_mode": normalize_deployment_mode(raw.get("deployment_mode").and_then(Value::as_str)),
        "certificates": certificates
    })
}

pub(super) fn normalize_managed_certificate(value: Value) -> Option<Value> {
    let cert = value
        .get("cert")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    let key = value.get("key").and_then(Value::as_str)?.trim().to_string();
    if cert.is_empty() || key.is_empty() {
        return None;
    }
    let source = normalize_certificate_source(value.get("source").and_then(Value::as_str));
    let primary_domain = value
        .get("primary_domain")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| build_ssl_certificate_id(&cert, &key));
    let label = value
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_certificate_label(source, primary_domain.as_deref()));
    Some(json!({
        "id": id,
        "label": label,
        "source": source,
        "primary_domain": primary_domain,
        "source_ref_id": optional_string(value.get("source_ref_id")),
        "cert": cert,
        "key": key,
        "created_at": normalize_timestamp(value.get("created_at")).unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        "updated_at": normalize_timestamp(value.get("updated_at")).unwrap_or_else(|| normalize_timestamp(value.get("created_at")).unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()))
    }))
}

pub(super) fn mirror_active_ssl_certificate(ssl: &Value, active_id: Option<&str>) -> Value {
    let normalized = normalize_ssl_config(Some(ssl));
    let active = active_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|id| {
            normalized
                .get("certificates")
                .and_then(Value::as_array)?
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        });
    let active_cert = active
        .and_then(|item| item.get("cert").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let active_key = active
        .and_then(|item| item.get("key").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let active_id = active
        .and_then(|item| item.get("id").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let mut next = normalized;
    next["cert"] = json!(active_cert);
    next["key"] = json!(active_key);
    next["active_cert_id"] = json!(active_id);
    next
}

pub(super) fn validate_ssl_cert(cert: &str, key: &str) -> anyhow::Result<()> {
    validate_ssl_cert_pair(cert, key).map_err(|error| anyhow!(ssl_validation_error_plain(error)))
}

pub(crate) fn parse_cert_info(cert_pem: &str) -> Option<Value> {
    let (_, pem) = parse_x509_pem(cert_pem.as_bytes()).ok()?;
    let cert = pem.parse_x509().ok()?;
    let mut dns_names = Vec::new();
    if let Ok(Some(san)) = cert.subject_alternative_name() {
        for name in &san.value.general_names {
            match name {
                GeneralName::DNSName(value) => dns_names.push(value.to_string()),
                GeneralName::IPAddress(bytes) => {
                    if bytes.len() == 4 {
                        dns_names.push(format!(
                            "{}.{}.{}.{}",
                            bytes[0], bytes[1], bytes[2], bytes[3]
                        ));
                    } else if bytes.len() == 16 {
                        let mut segments = [0_u16; 8];
                        for (index, chunk) in bytes.chunks(2).enumerate().take(8) {
                            segments[index] = u16::from_be_bytes([chunk[0], chunk[1]]);
                        }
                        dns_names.push(
                            std::net::Ipv6Addr::new(
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
                        );
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(cn) = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(str::to_string)
    {
        if !dns_names
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(&cn))
        {
            dns_names.push(cn);
        }
    }
    Some(json!({
        "issuer": cert.issuer().to_string(),
        "subject": cert.subject().to_string(),
        "validFrom": cert.validity().not_before.to_string(),
        "validTo": cert.validity().not_after.to_string(),
        "dnsNames": dns_names,
        "serialNumber": cert.raw_serial_as_string()
    }))
}
