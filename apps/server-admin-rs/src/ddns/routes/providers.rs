use super::*;
mod alidns;
mod baidu;
mod cloudflare;
mod common;
mod dnspod;
mod dynu;
mod edgeone;
mod esa_dynv6;
mod godaddy;
mod huawei;
mod porkbun;
mod simple;
mod tencentcloud;

pub(super) use alidns::*;
pub(super) use baidu::*;
pub(super) use cloudflare::*;
pub(super) use common::*;
pub(super) use dnspod::*;
pub(super) use dynu::*;
pub(super) use edgeone::*;
pub(super) use esa_dynv6::*;
pub(super) use godaddy::*;
pub(super) use huawei::*;
pub(super) use porkbun::*;
pub(super) use simple::*;
pub(super) use tencentcloud::*;

pub(super) async fn update_ddns_provider(
    translator: &Translator,
    provider: &str,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    if !is_known_ddns_provider(provider) {
        return update_ddns_provider_once(translator, provider, config, http_options, ipv4, ipv6)
            .await;
    }

    let retry_options = ddns_provider_retry_options_from_env();
    if retry_options.max_attempts == 0 {
        return Ok(provider_failure("null"));
    }

    let mut last_error = None;
    for attempt in 1..=retry_options.max_attempts {
        match update_ddns_provider_once(translator, provider, config, http_options, ipv4, ipv6)
            .await
        {
            Ok(result) => return Ok(result),
            Err(error) => {
                last_error = Some(error);
                if attempt < retry_options.max_attempts {
                    tokio_time::sleep(Duration::from_millis(retry_options.delay_ms)).await;
                }
            }
        }
    }

    Ok(provider_failure(
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "null".to_string()),
    ))
}

pub(super) fn is_known_ddns_provider(provider: &str) -> bool {
    matches!(
        provider,
        "alidns"
            | "baiducloud"
            | "cloudflare"
            | "dnspod"
            | "duckdns"
            | "dynu"
            | "edgeone"
            | "edgeone_cname"
            | "esa"
            | "godaddy"
            | "huaweicloud"
            | "noip"
            | "porkbun"
            | "tencentcloud"
            | "dynv6"
    )
}

async fn update_ddns_provider_once(
    translator: &Translator,
    provider: &str,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    match provider {
        "alidns" => update_alidns(translator, config, http_options, ipv4, ipv6).await,
        "baiducloud" => update_baiducloud(translator, config, http_options, ipv4, ipv6).await,
        "cloudflare" => update_cloudflare(translator, config, http_options, ipv4, ipv6).await,
        "dnspod" => update_dnspod(translator, config, http_options, ipv4, ipv6).await,
        "duckdns" => update_duckdns(translator, config, http_options, ipv4, ipv6).await,
        "dynu" => update_dynu(translator, config, http_options, ipv4, ipv6).await,
        "edgeone" => update_edgeone(translator, config, http_options, ipv4, ipv6).await,
        "edgeone_cname" => update_edgeone_cname(translator, config, http_options, ipv4, ipv6).await,
        "esa" => update_esa(translator, config, http_options, ipv4, ipv6).await,
        "godaddy" => update_godaddy(translator, config, http_options, ipv4, ipv6).await,
        "huaweicloud" => update_huaweicloud(translator, config, http_options, ipv4, ipv6).await,
        "noip" => update_noip(translator, config, http_options, ipv4, ipv6).await,
        "porkbun" => update_porkbun(translator, config, http_options, ipv4, ipv6).await,
        "tencentcloud" => update_tencentcloud(translator, config, http_options, ipv4, ipv6).await,
        "dynv6" => update_dynv6(translator, config, http_options, ipv4, ipv6).await,
        other => Ok(DDNSProviderUpdateResult {
            success: false,
            message: ddns_text(
                translator,
                "unknownProvider",
                &[("provider", other.to_string())],
            ),
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DDNSProviderRetryOptions {
    max_attempts: usize,
    delay_ms: u64,
}

fn ddns_provider_retry_options_from_env() -> DDNSProviderRetryOptions {
    DDNSProviderRetryOptions {
        max_attempts: ddns_provider_retry_max_attempts(
            env::var("DDNS_RETRY_COUNT").ok().as_deref(),
        ),
        delay_ms: ddns_provider_retry_delay_ms(env::var("DDNS_RETRY_DELAY_MS").ok().as_deref()),
    }
}

pub(super) fn ddns_provider_retry_max_attempts(value: Option<&str>) -> usize {
    let retry_count = js_number_env_or_default(value, 1.0);
    if !retry_count.is_finite() {
        return 0;
    }
    (retry_count + 1.0).max(1.0).floor() as usize
}

pub(super) fn ddns_provider_retry_delay_ms(value: Option<&str>) -> u64 {
    let delay = js_number_env_or_default(value, 600.0);
    if !delay.is_finite() || delay <= 0.0 {
        0
    } else {
        delay.floor() as u64
    }
}

fn js_number_env_or_default(value: Option<&str>, fallback: f64) -> f64 {
    let raw = value.unwrap_or("");
    if raw.is_empty() {
        return fallback;
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        && !hex.is_empty()
        && let Ok(value) = u64::from_str_radix(hex, 16)
    {
        return value as f64;
    }
    if let Some(binary) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
        && !binary.is_empty()
        && let Ok(value) = u64::from_str_radix(binary, 2)
    {
        return value as f64;
    }
    if let Some(octal) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
        && !octal.is_empty()
        && let Ok(value) = u64::from_str_radix(octal, 8)
    {
        return value as f64;
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}
