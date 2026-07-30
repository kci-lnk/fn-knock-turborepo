use super::*;
mod alidns;
mod baidu;
mod cloudflare;
mod common;
mod dnshe;
mod dnspod;
mod duckdns;
mod dynu;
mod dynv6;
mod edgeone;
mod edgeone_cname;
mod edgeone_common;
mod esa;
mod godaddy;
mod huawei;
mod noip;
mod porkbun;
mod tencentcloud;
mod tencentcloud_tc3;

pub(super) use alidns::*;
pub(super) use baidu::*;
pub(super) use cloudflare::*;
pub(super) use common::*;
pub(super) use dnshe::*;
pub(super) use dnspod::*;
pub(super) use duckdns::*;
pub(super) use dynu::*;
pub(super) use dynv6::*;
pub(super) use edgeone::*;
pub(super) use edgeone_cname::*;
pub(super) use edgeone_common::*;
pub(super) use esa::*;
pub(super) use godaddy::*;
pub(super) use huawei::*;
pub(super) use noip::*;
pub(super) use porkbun::*;
pub(super) use tencentcloud::*;
pub(super) use tencentcloud_tc3::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DDNSDomainUpdatePlan {
    pub(super) config: HashMap<String, String>,
    pub(super) targets: Option<DdnsDomainTargets>,
    pub(super) execution: DdnsDomainUpdateExecution,
    preflight_complete: bool,
}

pub(super) fn build_ddns_provider_update_plan(
    provider: &str,
    config: &HashMap<String, String>,
) -> anyhow::Result<DDNSDomainUpdatePlan> {
    let mut normalized = config.clone();
    let targets = normalize_and_validate_ddns_domain_config(provider, &mut normalized)?;
    let execution = match targets.as_ref() {
        Some(targets) if targets.is_pair() => ddns_provider_domain_policy(provider)
            .map(|policy| policy.pair_execution)
            .unwrap_or(DdnsDomainUpdateExecution::Single),
        _ => DdnsDomainUpdateExecution::Single,
    };
    let preflight = targets
        .as_ref()
        .filter(|targets| targets.is_pair())
        .and_then(|_| ddns_provider_domain_policy(provider))
        .map(|policy| policy.preflight)
        .unwrap_or(DdnsDomainRootPreflight::None);
    Ok(DDNSDomainUpdatePlan {
        config: normalized,
        targets,
        execution,
        preflight_complete: !matches!(
            preflight,
            DdnsDomainRootPreflight::CloudflareZone
                | DdnsDomainRootPreflight::EdgeOneZone
                | DdnsDomainRootPreflight::EsaSite
        ),
    })
}

pub(super) async fn prepare_ddns_provider_update(
    translator: &Translator,
    provider: &str,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
) -> anyhow::Result<DDNSDomainUpdatePlan> {
    let mut plan = build_ddns_provider_update_plan(provider, config)?;
    preflight_ddns_provider_update(translator, provider, &mut plan, http_options).await?;
    Ok(plan)
}

pub(super) fn ddns_preflight_required_before_auxiliary(
    provider: &str,
    plan: &DDNSDomainUpdatePlan,
) -> bool {
    !plan.preflight_complete
        && ddns_provider_domain_policy(provider)
            .is_some_and(|policy| policy.preflight == DdnsDomainRootPreflight::EdgeOneZone)
}

pub(super) async fn preflight_ddns_provider_update(
    translator: &Translator,
    provider: &str,
    plan: &mut DDNSDomainUpdatePlan,
    http_options: &DDNSHttpClientOptions,
) -> anyhow::Result<()> {
    if plan.preflight_complete {
        return Ok(());
    }
    let root = plan
        .targets
        .as_ref()
        .and_then(DdnsDomainTargets::pair_root)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("missing DDNS pair root for preflight"))?;
    let preflight = ddns_provider_domain_policy(provider)
        .map(|policy| policy.preflight)
        .unwrap_or(DdnsDomainRootPreflight::None);
    match preflight {
        DdnsDomainRootPreflight::CloudflareZone => {
            validate_cloudflare_pair_root_in_zone(translator, &plan.config, http_options, &root)
                .await?;
        }
        DdnsDomainRootPreflight::EdgeOneZone => {
            validate_edgeone_pair_root_in_zone(translator, &plan.config, http_options, &root)
                .await?;
        }
        DdnsDomainRootPreflight::EsaSite => {
            let site_id =
                resolve_and_validate_esa_site_id(translator, &plan.config, http_options, &root)
                    .await?;
            plan.config.insert("site_id".to_string(), site_id);
        }
        DdnsDomainRootPreflight::None | DdnsDomainRootPreflight::DynuService => {}
    }
    plan.preflight_complete = true;
    Ok(())
}

pub(super) async fn execute_ddns_provider_update(
    translator: &Translator,
    provider: &str,
    plan: &DDNSDomainUpdatePlan,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    if !plan.preflight_complete {
        anyhow::bail!("DDNS domain update plan must be preflighted before execution");
    }
    match plan.execution {
        DdnsDomainUpdateExecution::Single => {
            update_ddns_provider_single(
                translator,
                provider,
                &plan.config,
                http_options,
                ipv4,
                ipv6,
            )
            .await
        }
        DdnsDomainUpdateExecution::DynuWildcardAlias => {
            let mut config = plan.config.clone();
            let wildcard = plan
                .targets
                .as_ref()
                .and_then(DdnsDomainTargets::wildcard)
                .ok_or_else(|| anyhow::anyhow!("missing Dynu wildcard target"))?;
            config.insert("domain".to_string(), wildcard.to_string());
            update_ddns_provider_single(translator, provider, &config, http_options, ipv4, ipv6)
                .await
        }
        DdnsDomainUpdateExecution::FanOut => {
            let targets = plan
                .targets
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing DDNS domain targets"))?;
            Ok(execute_ddns_domain_fanout(translator, targets, |domain| {
                let mut config = plan.config.clone();
                config.insert("domain".to_string(), domain);
                async move {
                    update_ddns_provider_single(
                        translator,
                        provider,
                        &config,
                        http_options,
                        ipv4,
                        ipv6,
                    )
                    .await
                }
            })
            .await)
        }
    }
}

pub(super) async fn execute_ddns_domain_fanout<F, Fut>(
    translator: &Translator,
    targets: &DdnsDomainTargets,
    mut update: F,
) -> DDNSProviderUpdateResult
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = anyhow::Result<DDNSProviderUpdateResult>>,
{
    let mut results = Vec::new();
    for domain in targets.domains() {
        let domain = domain.to_string();
        let result = match update(domain.clone()).await {
            Ok(result) => result,
            Err(error) => provider_failure(error.to_string()),
        };
        results.push((domain, result));
    }
    aggregate_domain_update_results(translator, results)
}

pub(super) fn aggregate_domain_update_results(
    translator: &Translator,
    results: Vec<(String, DDNSProviderUpdateResult)>,
) -> DDNSProviderUpdateResult {
    let success = results.iter().all(|(_, result)| result.success);
    if success {
        return DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(
                translator,
                "domainTargets.allSucceeded",
                &[("count", results.len().to_string())],
            ),
        };
    }
    let message = results
        .into_iter()
        .map(|(domain, result)| {
            if result.success {
                ddns_text(
                    translator,
                    "domainTargets.itemSucceeded",
                    &[("domain", domain)],
                )
            } else {
                let detail = result.message.trim();
                ddns_text(
                    translator,
                    "domainTargets.itemFailed",
                    &[
                        ("domain", domain),
                        (
                            "detail",
                            if detail.is_empty() {
                                ddns_text(translator, "requestFailed", &[])
                            } else {
                                detail.to_string()
                            },
                        ),
                    ],
                )
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    DDNSProviderUpdateResult { success, message }
}

async fn update_ddns_provider_single(
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
            | "dnshe"
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
        "dnshe" => update_dnshe(translator, config, http_options, ipv4, ipv6).await,
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
