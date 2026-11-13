pub(crate) mod cloudflared;
mod connectivity;
pub(crate) mod frpc;
pub(crate) mod supervisor;

pub(crate) const TUNNEL_RUNTIME_KEY: &str = "fn_knock:tunnel:runtime";
/// Loopback-only ingress exposed by the Go gateway for fn-knock-managed
/// Cloudflare Tunnels. Keep service discovery and managed tunnel reconciliation
/// on this shared value so the private ingress can never be advertised as an
/// ordinary upstream service.
pub(crate) const MANAGED_CLOUDFLARE_INGRESS_PORT: u16 = 17_999;

/// Lite can coexist with the full fnOS package on the same host.
pub(crate) const MANAGED_CLOUDFLARE_LITE_INGRESS_PORT: u16 = 18_999;

pub(crate) fn managed_cloudflare_ingress_port(
    target: &str,
    configured: &str,
) -> anyhow::Result<u16> {
    if configured.is_empty() {
        return Ok(if target == "fpk-lite" {
            MANAGED_CLOUDFLARE_LITE_INGRESS_PORT
        } else {
            MANAGED_CLOUDFLARE_INGRESS_PORT
        });
    }
    match configured.parse::<u16>() {
        Ok(port) if port > 0 && port.to_string() == configured => Ok(port),
        _ => anyhow::bail!(
            "FN_KNOCK_CLOUDFLARE_INGRESS_PORT must be a decimal port between 1 and 65535"
        ),
    }
}

#[cfg(test)]
mod ingress_tests {
    use super::managed_cloudflare_ingress_port as port;

    #[test]
    fn configured_port_and_platform_defaults() {
        assert_eq!(port("fpk", "").unwrap(), 17999);
        assert_eq!(port("docker", "").unwrap(), 17999);
        assert_eq!(port("fpk-lite", "").unwrap(), 18999);
        assert_eq!(port("docker", "27999").unwrap(), 27999);
        for invalid in ["0", "65536", "-1", " 17999", "010", "http"] {
            assert!(port("fpk", invalid).is_err());
        }
    }
}
