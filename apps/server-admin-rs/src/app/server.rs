use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use anyhow::Context;
use axum::Router;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub(super) struct BoundServer {
    name: &'static str,
    listeners: Vec<(SocketAddr, TcpListener)>,
    router: Router,
    shutdown: CancellationToken,
}

impl BoundServer {
    pub(super) async fn bind(
        name: &'static str,
        addr: SocketAddr,
        router: Router,
        shutdown: CancellationToken,
    ) -> anyhow::Result<Self> {
        let listeners = bind_listeners(name, addr, TcpListener::bind).await?;

        Ok(Self {
            name,
            listeners,
            router,
            shutdown,
        })
    }

    pub(super) async fn serve(self) -> anyhow::Result<()> {
        let mut tasks = tokio::task::JoinSet::new();
        for (listen_addr, listener) in self.listeners {
            let router = self.router.clone();
            let shutdown = self.shutdown.clone();
            let name = self.name;
            tasks.spawn(async move {
                serve_listener(name, listen_addr, listener, router, shutdown).await
            });
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
}

// The configured address is always the primary listener. When it is a
// loopback address, the other IP family is a convenience companion rather
// than a startup requirement: Windows installations can legitimately have
// IPv6 disabled. Keep the primary listener available in that case instead of
// dropping it because the companion cannot bind.
async fn bind_listeners<T, F, Fut>(
    name: &'static str,
    addr: SocketAddr,
    mut bind: F,
) -> anyhow::Result<Vec<(SocketAddr, T)>>
where
    F: FnMut(SocketAddr) -> Fut,
    Fut: Future<Output = std::io::Result<T>>,
{
    let mut listeners = Vec::new();
    for (index, listen_addr) in listen_addrs(addr).into_iter().enumerate() {
        match bind(listen_addr).await {
            Ok(listener) => {
                tracing::info!(%name, addr = %listen_addr, "server listening");
                listeners.push((listen_addr, listener));
            }
            Err(error) if index > 0 => {
                tracing::warn!(
                    %name,
                    addr = %listen_addr,
                    %error,
                    "optional loopback companion listener unavailable; continuing with the primary listener"
                );
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("bind {name} listener on {listen_addr}"));
            }
        }
    }
    Ok(listeners)
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
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown.cancelled_owned())
    .await
    .with_context(|| format!("{name} server failed on {addr}"))?;
    Ok(())
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

    #[tokio::test]
    async fn unavailable_loopback_companion_keeps_primary_listener() {
        let primary = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7997);
        let companion = loopback_companion_addr(primary).expect("loopback has a companion");

        let listeners = bind_listeners("test", primary, |listen_addr| async move {
            if listen_addr == companion {
                Err(std::io::Error::new(
                    std::io::ErrorKind::AddrNotAvailable,
                    "IPv6 is disabled",
                ))
            } else {
                Ok(listen_addr)
            }
        })
        .await
        .expect("primary listener should still bind");

        assert_eq!(listeners, vec![(primary, primary)]);
    }

    #[tokio::test]
    async fn unavailable_primary_listener_still_fails_startup() {
        let primary = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7997);
        let error = bind_listeners("test", primary, |_| async {
            Err::<SocketAddr, _>(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "port is in use",
            ))
        })
        .await
        .expect_err("primary listener failure must remain fatal");

        assert!(error.to_string().contains("bind test listener"));
    }
}
