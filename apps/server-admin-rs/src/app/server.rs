use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use anyhow::Context;
use axum::Router;
use tokio::net::TcpListener;

pub(super) async fn serve(
    name: &'static str,
    addr: SocketAddr,
    router: Router,
) -> anyhow::Result<()> {
    let mut listeners = Vec::new();
    for listen_addr in listen_addrs(addr) {
        let listener = TcpListener::bind(listen_addr)
            .await
            .with_context(|| format!("bind {name} listener on {listen_addr}"))?;
        tracing::info!(%name, addr = %listen_addr, "server listening");
        listeners.push((listen_addr, listener));
    }

    let mut tasks = tokio::task::JoinSet::new();
    for (listen_addr, listener) in listeners {
        let router = router.clone();
        tasks.spawn(async move { serve_listener(name, listen_addr, listener, router).await });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tasks.abort_all();
                return Err(error);
            }
            Err(error) => {
                tasks.abort_all();
                return Err(error.into());
            }
        }
    }

    Ok(())
}

fn listen_addrs(addr: SocketAddr) -> Vec<SocketAddr> {
    let mut addrs = vec![addr];
    if let Some(companion) = loopback_companion_addr(addr) {
        addrs.push(companion);
    }
    addrs
}

fn loopback_companion_addr(addr: SocketAddr) -> Option<SocketAddr> {
    match addr.ip() {
        IpAddr::V4(ip) if ip.is_loopback() => Some(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            addr.port(),
        )),
        IpAddr::V6(ip) if ip.is_loopback() => Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            addr.port(),
        )),
        _ => None,
    }
}

async fn serve_listener(
    name: &'static str,
    addr: SocketAddr,
    listener: TcpListener,
    router: Router,
) -> anyhow::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .with_context(|| format!("{name} server failed on {addr}"))?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_listeners_include_ipv4_and_ipv6_without_wildcard() {
        let ipv4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7997);
        assert_eq!(
            listen_addrs(ipv4),
            vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7997),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 7997),
            ]
        );

        let ipv6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 7998);
        assert_eq!(
            listen_addrs(ipv6),
            vec![
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 7998),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7998),
            ]
        );

        let wildcard = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 7997);
        assert_eq!(listen_addrs(wildcard), vec![wildcard]);

        let ipv6_wildcard = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 7997);
        assert_eq!(listen_addrs(ipv6_wildcard), vec![ipv6_wildcard]);
    }
}
