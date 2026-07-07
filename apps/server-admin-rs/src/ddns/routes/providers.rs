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
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    match provider {
        "alidns" => update_alidns(translator, config, ipv4, ipv6).await,
        "baiducloud" => update_baiducloud(translator, config, ipv4, ipv6).await,
        "cloudflare" => update_cloudflare(translator, config, ipv4, ipv6).await,
        "dnspod" => update_dnspod(translator, config, ipv4, ipv6).await,
        "duckdns" => update_duckdns(translator, config, ipv4, ipv6).await,
        "dynu" => update_dynu(translator, config, ipv4, ipv6).await,
        "edgeone" => update_edgeone(translator, config, ipv4, ipv6).await,
        "edgeone_cname" => update_edgeone_cname(translator, config, ipv4, ipv6).await,
        "esa" => update_esa(translator, config, ipv4, ipv6).await,
        "godaddy" => update_godaddy(translator, config, ipv4, ipv6).await,
        "huaweicloud" => update_huaweicloud(translator, config, ipv4, ipv6).await,
        "noip" => update_noip(translator, config, ipv4, ipv6).await,
        "porkbun" => update_porkbun(translator, config, ipv4, ipv6).await,
        "tencentcloud" => update_tencentcloud(translator, config, ipv4, ipv6).await,
        "dynv6" => update_dynv6(translator, config, ipv4, ipv6).await,
        other => Ok(DDNSProviderUpdateResult {
            success: false,
            message: ddns_text(
                translator,
                "unknownProvider",
                &[("provider", other.to_string())],
            ),
            ipv4_updated: false,
            ipv6_updated: false,
        }),
    }
}
