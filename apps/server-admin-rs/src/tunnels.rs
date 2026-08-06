pub(crate) mod cloudflared;
mod connectivity;
pub(crate) mod frpc;
pub(crate) mod supervisor;

pub(crate) const TUNNEL_RUNTIME_KEY: &str = "fn_knock:tunnel:runtime";
