use super::*;
use crate::app_version::{APP_GITHUB_URL, APP_LOCAL_VERSION};

pub(in crate::ddns::routes) fn noip_catalog_entry() -> Value {
    provider(
        "noip",
        "NO-IP",
        vec![
            field("hostname", "Hostname", "text", "home.ddns.net", true),
            field("username", "Username", "text", "DDNS Key Username", true),
            field(
                "password",
                "Password",
                "password",
                "DDNS Key Password",
                true,
            ),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_noip(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let hostname = config_value(config, "hostname");
    let username = config_value(config, "username");
    let password = config_value(config, "password");
    if hostname.is_empty() || username.is_empty() || password.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.noIpAvailable",
            &[],
        )));
    }
    let mut query = vec![("hostname", hostname)];
    let combined;
    if let (Some(ipv4), Some(ipv6)) = (ipv4, ipv6) {
        combined = format!("{ipv4},{ipv6}");
        query.push(("myip", combined.clone()));
    } else if let Some(ipv4) = ipv4 {
        query.push(("myip", ipv4.to_string()));
    } else if let Some(ipv6) = ipv6 {
        query.push(("myipv6", ipv6.to_string()));
    }
    let client = ddns_http_client(translator, http_options)
        .map_err(|error| provider_request_error(translator, "noip", error))?;
    let authorization = BASE64_STANDARD.encode(format!("{username}:{password}"));
    let response = client
        .get(build_query_url(
            "https://dynupdate.no-ip.com/nic/update",
            &query,
        ))
        .header(reqwest::header::ACCEPT, "text/plain")
        .header(reqwest::header::USER_AGENT, noip_user_agent())
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Basic {authorization}"),
        )
        .send()
        .await
        .map_err(|error| provider_request_error(translator, "noip", error))?;
    let status = response.status();
    let text = response_text(response)
        .await
        .map_err(|error| provider_request_error(translator, "noip", error))?;
    if !status.is_success() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.updateFailedWithStatus",
            &[
                ("status", status.as_u16().to_string()),
                (
                    "detail",
                    if text.is_empty() {
                        ddns_text(translator, "providers.noip.requestFailed", &[])
                    } else {
                        text
                    },
                ),
            ],
        )));
    }
    let statuses = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut parts = line.split_whitespace();
            let code = parts.next().unwrap_or("").to_string();
            let detail = parts.collect::<Vec<_>>().join(" ");
            (code, detail)
        })
        .collect::<Vec<_>>();
    if statuses.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.emptyResponse",
            &[],
        )));
    }
    let failures = statuses
        .iter()
        .filter(|(code, _)| code != "good" && code != "nochg")
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        let detail = failures
            .into_iter()
            .map(|(code, detail)| noip_status_message(translator, code, detail))
            .collect::<Vec<_>>()
            .join("; ");
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.updateFailed",
            &[("detail", detail)],
        )));
    }
    let changed = statuses.iter().any(|(code, _)| code == "good");
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: if changed {
            ddns_text(
                translator,
                "providers.noip.updateSuccess",
                &[("detail", noip_detail_suffix(&statuses))],
            )
        } else {
            ddns_text(
                translator,
                "providers.noip.ipUnchanged",
                &[("detail", noip_detail_suffix(&statuses))],
            )
        },
    })
}

pub(in crate::ddns::routes) fn noip_status_message(
    translator: &Translator,
    code: &str,
    raw_detail: &str,
) -> String {
    let known = matches!(
        code,
        "nohost" | "badauth" | "badagent" | "!donator" | "abuse" | "911"
    );
    let reason = if known {
        ddns_text(
            translator,
            &format!("providers.noip.statusMessages.{code}"),
            &[],
        )
    } else if raw_detail.is_empty() {
        ddns_text(
            translator,
            "providers.noip.unknownStatus",
            &[("code", code.to_string())],
        )
    } else {
        raw_detail.to_string()
    };
    if known && !raw_detail.is_empty() {
        format!("{code} ({reason}; {raw_detail})")
    } else {
        format!("{code} ({reason})")
    }
}

pub(in crate::ddns::routes) fn noip_detail_suffix(statuses: &[(String, String)]) -> String {
    let details = statuses
        .iter()
        .filter_map(|(_, detail)| {
            let detail = detail.trim();
            (!detail.is_empty()).then(|| detail.to_string())
        })
        .collect::<Vec<_>>();
    if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join("; "))
    }
}

pub(in crate::ddns::routes) fn noip_user_agent() -> String {
    format!("fn-knock/{APP_LOCAL_VERSION} ({APP_GITHUB_URL})")
}
