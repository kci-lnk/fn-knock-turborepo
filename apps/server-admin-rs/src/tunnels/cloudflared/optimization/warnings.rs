use serde_json::{Value, json};

const PLAN_WARNINGS: [(&str, &str); 4] = [
    (
        "betaVantage",
        "Optimization is a Beta feature measured from this server's network vantage point.",
    ),
    (
        "candidateDiscoveryOnly",
        "Built-in and custom third-party hostnames are used only to discover candidate Cloudflare IPs. Business DNS is never pointed at those hostnames.",
    ),
    (
        "customHostnameQuota",
        "Cloudflare for SaaS includes up to 100 exact Custom Hostnames on non-Enterprise plans; excess domains use the wildcard Tunnel.",
    ),
    (
        "wildcardFallback",
        "The wildcard Tunnel remains configured and is restored automatically if the preferred edge path fails.",
    ),
];

pub(in super::super) fn plan_warnings(enabled: bool) -> Vec<Value> {
    if !enabled {
        return Vec::new();
    }
    PLAN_WARNINGS
        .iter()
        .map(|(_, message)| json!(message))
        .collect()
}

pub(in super::super) fn plan_warning_codes(enabled: bool) -> Vec<&'static str> {
    if !enabled {
        return Vec::new();
    }
    PLAN_WARNINGS.iter().map(|(code, _)| *code).collect()
}
