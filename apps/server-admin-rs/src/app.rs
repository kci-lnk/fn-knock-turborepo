use std::{env, future::Future, path::PathBuf, pin::Pin, time::Duration};

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod boot;
mod cli;
mod docker_admin_view;
mod router;
mod server;

pub(crate) use boot::cleanup_legacy_auth_log_storage;

use crate::{
    acme::start_acme_tasks,
    auth::start_auth_bridge,
    auth_mobility::start_auth_mobility_tasks,
    auto_https::sync_auto_https_on_boot,
    cidr::migrate_cidr_query_caches_on_boot,
    cloudflared::start_cloudflared_tasks,
    common_auth_locations::{
        migrate_common_auth_location_ipset_on_boot, start_common_auth_location_tasks,
    },
    dashboard::start_traffic_tasks,
    ddns_status::start_ddns_tasks,
    fnos_certificate_sync::start_fnos_certificate_sync_tasks,
    frpc::start_frpc_tasks,
    gateway_settings::migrate_visibility_policies_on_boot,
    i18n::{DEFAULT_LOCALE, Translator},
    ip_location::start_ip_location_worker,
    maintenance::start_automatic_backup_tasks,
    memory,
    notifications::start_notification_tasks,
    runtime_profile,
    scanner::migrate_scanner_cidr_ipset_on_boot,
    settings::Settings,
    ssh_security::{migrate_ssh_ipset_on_boot, start_ssh_security_tasks},
    state::AppState,
    system_assets::start_system_clock_tasks,
    system_monitor::start_system_monitor_tasks,
    terminal::start_terminal_tasks,
    update::start_update_tasks,
    waf::start_waf_tasks,
    whitelist::{migrate_whitelist_ipsets_on_boot, start_whitelist_tasks},
};

pub async fn run() -> anyhow::Result<()> {
    init_tracing();

    if let Some(command) = env::args().nth(1) {
        match command.as_str() {
            "reset-panel-password" | "reset-admin-panel-password" => {
                if let Err(error) = reset_panel_password_command().await {
                    let locale =
                        env::var("FN_KNOCK_LOCALE").unwrap_or_else(|_| DEFAULT_LOCALE.to_string());
                    let translator = Translator::new(locale);
                    eprintln!(
                        "{} {}",
                        translator.t("server.dockerAdminPanel.resetFailed"),
                        error
                    );
                    std::process::exit(1);
                }
                return Ok(());
            }
            "migrate-redis-to-sqlite" => {
                if let Err(error) = cli::migrate_redis_to_sqlite_command().await {
                    eprintln!("legacy Redis migration failed: {error}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            "-h" | "--help" => {
                cli::print_help();
                return Ok(());
            }
            _ => anyhow::bail!("unknown command: {command}"),
        }
    }

    let shutdown = CancellationToken::new();
    let signal_shutdown = shutdown.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        signal_shutdown.cancel();
    });
    run_with_settings(Settings::from_env(), shutdown, None).await
}

pub(crate) async fn reset_panel_password_command() -> anyhow::Result<()> {
    cli::reset_panel_password_command().await
}

pub(crate) fn init_tracing() {
    let _ =
        tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("server_admin_rs=info,tower_http=info,axum=info")
            }))
            .with(tracing_subscriber::fmt::layer())
            .try_init();
}

pub(crate) async fn run_with_settings(
    settings: Settings,
    shutdown: CancellationToken,
    ready: Option<oneshot::Sender<()>>,
) -> anyhow::Result<()> {
    let readiness_marker = env::var_os("FN_KNOCK_READY_FILE").map(PathBuf::from);
    if let Some(path) = &readiness_marker {
        let _ = tokio::fs::remove_file(path).await;
    }
    // Child cancellation propagates an SCM/signal stop into every listener and
    // worker, while an application startup error can tear down its own tasks
    // without masquerading as an external service stop in the supervisor.
    let runtime_shutdown = shutdown.child_token();
    let state = AppState::new_with_shutdown(settings.clone(), runtime_shutdown.clone()).await?;
    wait_for_gateway_control_plane(&state, &runtime_shutdown, Duration::from_secs(60)).await?;
    let migrated_cidr_caches = migrate_cidr_query_caches_on_boot(&state).await?;
    if migrated_cidr_caches > 0 {
        tracing::info!(
            migrated_cidr_caches,
            "migrated CIDR query caches to compiled policies"
        );
    }
    migrate_visibility_policies_on_boot(&state)
        .await
        .map_err(anyhow::Error::msg)?;
    migrate_scanner_cidr_ipset_on_boot(&state).await?;
    migrate_common_auth_location_ipset_on_boot(&state).await?;
    migrate_whitelist_ipsets_on_boot(&state).await?;
    migrate_ssh_ipset_on_boot(&state).await?;
    sync_auto_https_on_boot(state.clone()).await;
    boot::start_boot_sync_tasks(state.clone());
    start_traffic_tasks(state.clone());
    start_ip_location_worker(state.clone());
    start_notification_tasks(state.clone());
    start_automatic_backup_tasks(state.clone());
    let profile = runtime_profile::get_runtime_profile(&state);
    let capabilities = runtime_profile::get_runtime_capabilities(&profile);
    start_update_tasks(state.clone());
    start_ddns_tasks(state.clone());
    if capabilities.acme_available {
        start_acme_tasks(state.clone());
    }
    if capabilities.fnos_certificate_sync_available {
        start_fnos_certificate_sync_tasks(state.clone());
    }
    if capabilities.system_resource_monitor_available {
        start_system_monitor_tasks(state.clone());
    }
    if capabilities.system_clock_sync_available {
        start_system_clock_tasks(state.clone());
    }
    start_auth_mobility_tasks(state.clone());
    start_common_auth_location_tasks(state.clone());
    start_whitelist_tasks(state.clone());
    start_waf_tasks(state.clone());
    if capabilities.terminal_available {
        start_terminal_tasks(state.clone());
    }
    if capabilities.ssh_security_available {
        start_ssh_security_tasks(state.clone());
    }
    memory::trim_allocated_memory_after(Duration::from_secs(45));

    let backend_addr = settings.backend_addr()?;
    let auth_addr = settings.auth_addr()?;
    let protected_admin_runtime = runtime_profile::admin_panel_protected_runtime(&state);
    let admin_view_addr = if protected_admin_runtime {
        settings.admin_view_addr()?
    } else {
        None
    };

    let backend = server::BoundServer::bind(
        "backend",
        backend_addr,
        router::backend_router(state.clone(), false),
        runtime_shutdown.clone(),
    )
    .await?;
    let auth = server::BoundServer::bind(
        "auth",
        auth_addr,
        router::auth_router(state.clone()),
        runtime_shutdown.clone(),
    )
    .await?;

    let admin_view = if let Some(addr) = admin_view_addr {
        Some(
            server::BoundServer::bind(
                "admin-view",
                addr,
                router::backend_router(state.clone(), true),
                runtime_shutdown.clone(),
            )
            .await?,
        )
    } else {
        None
    };
    // Tunnel supervisors are started only after every listener has bound.
    // This keeps a startup bind error from dropping the runtime while managed
    // tunnel children are already alive.
    if capabilities.cloudflared_available {
        start_cloudflared_tasks(state.clone());
    }
    if capabilities.frpc_available {
        start_frpc_tasks(state.clone());
    }
    let servers = async move {
        if let Some(admin_view) = admin_view {
            tokio::try_join!(backend.serve(), auth.serve(), admin_view.serve())?;
        } else {
            tokio::try_join!(backend.serve(), auth.serve())?;
        }
        Ok(())
    };
    tokio::pin!(servers);
    let auth_bridge = start_auth_bridge(state.clone());

    // Poll the listeners while waiting for the bridge. This ordering is
    // important on Windows: the Go data plane may issue auth requests as soon
    // as the bridge handshakes, and SCM Running must still be withheld until
    // bundle + process + data plane + auth bridge are all healthy.
    let readiness = wait_for_readiness_while_serving(
        servers.as_mut(),
        wait_for_gateway(&state, &runtime_shutdown, Some(Duration::from_secs(60))),
    )
    .await;
    if let Err(error) = readiness {
        runtime_shutdown.cancel();
        state
            .tunnel_supervisors
            .shutdown_all(Duration::from_secs(10))
            .await;
        stop_auth_bridge(auth_bridge).await;
        if let Some(path) = &readiness_marker {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(error);
    }
    if let Some(path) = &readiness_marker {
        tokio::fs::write(path, b"ready\n").await?;
    }
    if let Some(ready) = ready {
        let _ = ready.send(());
    }
    let result = servers.await;
    runtime_shutdown.cancel();
    state
        .tunnel_supervisors
        .shutdown_all(Duration::from_secs(10))
        .await;
    stop_auth_bridge(auth_bridge).await;
    if let Some(path) = &readiness_marker {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}

async fn stop_auth_bridge(mut auth_bridge: tokio::task::JoinHandle<()>) {
    if tokio::time::timeout(Duration::from_secs(5), &mut auth_bridge)
        .await
        .is_err()
    {
        auth_bridge.abort();
        let _ = auth_bridge.await;
        tracing::warn!("auth bridge did not stop within the shutdown deadline");
    }
}

async fn wait_for_readiness_while_serving<S, R>(
    mut servers: Pin<&mut S>,
    readiness: R,
) -> anyhow::Result<()>
where
    S: Future<Output = anyhow::Result<()>>,
    R: Future<Output = anyhow::Result<()>>,
{
    tokio::pin!(readiness);
    tokio::select! {
        ready = &mut readiness => ready,
        result = &mut servers => match result {
            Ok(()) => anyhow::bail!("HTTP listeners stopped before runtime readiness"),
            Err(error) => Err(error),
        },
    }
}

async fn wait_for_gateway_control_plane(
    state: &AppState,
    shutdown: &CancellationToken,
    timeout: Duration,
) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let process_ready = state
            .go_backend
            .health_serving(crate::go_backend::GATEWAY_HEALTH_PROCESS)
            .await
            .unwrap_or(false);
        if process_ready {
            match state.go_backend.verify_bundle_compatibility().await {
                Ok(_) => return Ok(()),
                Err(crate::go_backend::BundleCompatibilityError::Unavailable(error)) => {
                    tracing::debug!(%error, "gateway compatibility probe is temporarily unavailable");
                }
                Err(error @ crate::go_backend::BundleCompatibilityError::Incompatible(_)) => {
                    return Err(error.into());
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("Go gateway control plane did not become ready within 60 seconds");
        }
        tokio::select! {
            _ = shutdown.cancelled() => anyhow::bail!("startup cancelled"),
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
    }
}

async fn wait_for_gateway(
    state: &AppState,
    shutdown: &CancellationToken,
    timeout: Option<Duration>,
) -> anyhow::Result<()> {
    let deadline = timeout.map(|timeout| tokio::time::Instant::now() + timeout);
    loop {
        let process_ready = state
            .go_backend
            .health_serving(crate::go_backend::GATEWAY_HEALTH_PROCESS)
            .await
            .unwrap_or(false);
        let dataplane_ready = state
            .go_backend
            .health_serving(crate::go_backend::GATEWAY_HEALTH_DATAPLANE)
            .await
            .unwrap_or(false);
        let auth_bridge_ready = state
            .go_backend
            .health_serving(crate::go_backend::GATEWAY_HEALTH_AUTH_BRIDGE)
            .await
            .unwrap_or(false);
        let bundle_ready = if process_ready {
            match state.go_backend.verify_bundle_compatibility().await {
                Ok(_) => true,
                Err(crate::go_backend::BundleCompatibilityError::Unavailable(error)) => {
                    tracing::debug!(%error, "gateway compatibility probe is temporarily unavailable");
                    false
                }
                Err(error @ crate::go_backend::BundleCompatibilityError::Incompatible(_)) => {
                    return Err(error.into());
                }
            }
        } else {
            false
        };
        if bundle_ready && dataplane_ready && auth_bridge_ready {
            return Ok(());
        }
        if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
            anyhow::bail!("Go gateway did not become ready within the startup deadline");
        }
        tokio::select! {
            _ = shutdown.cancelled() => anyhow::bail!("startup cancelled"),
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
    }
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
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    #[tokio::test]
    async fn listeners_are_polled_while_windows_readiness_is_pending() {
        let polled = Arc::new(AtomicBool::new(false));
        let server_polled = polled.clone();
        let servers = std::future::poll_fn(move |_| {
            server_polled.store(true, Ordering::SeqCst);
            std::task::Poll::<anyhow::Result<()>>::Pending
        });
        tokio::pin!(servers);
        let readiness = async move {
            tokio::task::yield_now().await;
            if !polled.load(Ordering::SeqCst) {
                anyhow::bail!("server future was not active during readiness check");
            }
            Ok(())
        };

        wait_for_readiness_while_serving(servers.as_mut(), readiness)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn tracked_auth_bridge_task_is_awaited_after_cancellation() {
        let shutdown = CancellationToken::new();
        let worker_shutdown = shutdown.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = stopped.clone();
        let worker = tokio::spawn(async move {
            worker_shutdown.cancelled().await;
            worker_stopped.store(true, Ordering::SeqCst);
        });

        shutdown.cancel();
        stop_auth_bridge(worker).await;

        assert!(stopped.load(Ordering::SeqCst));
    }
}
