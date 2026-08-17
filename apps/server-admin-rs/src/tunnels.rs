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
