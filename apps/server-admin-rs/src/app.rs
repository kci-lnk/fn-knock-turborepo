use std::{env, future::Future, path::PathBuf, pin::Pin, time::Duration};

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod boot;
mod cli;
mod docker_admin_view;
mod router;
mod server;
mod startup_gateway;

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
    runtime_health::{install_panic_hook, start_runtime_monitor},
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
    wol::start_wol_tasks,
};

// DSM gives the package supervisor 180 seconds by default. Keep one shared
// application budget so individually bounded startup phases cannot add up past
// that window, while retaining time for error propagation and process cleanup.
const DEFAULT_APPLICATION_STARTUP_TIMEOUT: Duration = Duration::from_secs(150);
const SYNOLOGY_SUPERVISOR_SHUTDOWN_MARGIN: Duration = Duration::from_secs(30);
const GATEWAY_STARTUP_PHASE_TIMEOUT: Duration = Duration::from_secs(60);

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
    mut settings: Settings,
    shutdown: CancellationToken,
    ready: Option<oneshot::Sender<()>>,
) -> anyhow::Result<()> {
    install_panic_hook(&settings.data_dir);
    let readiness_marker = env::var_os("FN_KNOCK_READY_FILE").map(PathBuf::from);
    if let Some(path) = &readiness_marker {
        let _ = tokio::fs::remove_file(path).await;
    }
    let startup_deadline = tokio::time::Instant::now() + application_startup_timeout();
    settings.ensure_altcha_hmac_key()?;
    // Child cancellation propagates an SCM/signal stop into every listener and
    // worker, while an application startup error can tear down its own tasks
    // without masquerading as an external service stop in the supervisor.
    let runtime_shutdown = shutdown.child_token();
    let state = AppState::new_with_shutdown(settings.clone(), runtime_shutdown.clone()).await?;
    wait_for_gateway_control_plane(
        &state,
        &runtime_shutdown,
        startup_phase_timeout(
            startup_deadline,
            GATEWAY_STARTUP_PHASE_TIMEOUT,
            "gateway control plane",
        )?,
    )
    .await?;
    // Apply the restored GOGC/memory-limit pair before expensive migrations,
    // listener readiness, or production traffic can create a startup spike.
    startup_gateway::sync_memory(
        &state,
        &runtime_shutdown,
        startup_phase_timeout(
            startup_deadline,
            GATEWAY_STARTUP_PHASE_TIMEOUT,
            "gateway memory configuration",
        )?,
    )
    .await?;
    start_runtime_monitor(state.clone()).await?;
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
    // The migration validates CIDR-bearing candidates, but even an empty
    // policy table can contain Host fields that require the matching gateway.
    // Publish the complete generation before listeners and readiness open.
    let startup_config = state.storage.store.get_config().await?;
    startup_gateway::sync_host_rules(
        &state,
        &startup_config,
        &runtime_shutdown,
        startup_phase_timeout(
            startup_deadline,
            GATEWAY_STARTUP_PHASE_TIMEOUT,
            "gateway host rules",
        )?,
    )
    .await?;
    migrate_scanner_cidr_ipset_on_boot(&state).await?;
    migrate_common_auth_location_ipset_on_boot(&state).await?;
    migrate_whitelist_ipsets_on_boot(&state).await?;
    migrate_ssh_ipset_on_boot(&state).await?;
    sync_auto_https_on_boot(state.clone()).await;
    let boot_sync_completed = boot::start_boot_sync_tasks(state.clone());
    start_traffic_tasks(state.clone());
    start_ip_location_worker(state.clone());
    start_notification_tasks(state.clone());
    start_automatic_backup_tasks(state.clone());
    start_wol_tasks(state.clone());
    let profile = runtime_profile::get_runtime_profile(&state);
    let capabilities = runtime_profile::get_runtime_capabilities(&profile);
    start_update_tasks(state.clone());
    start_ddns_tasks(state.clone());
    if capabilities.acme_available {
        start_acme_tasks(state.clone()).await;
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
    let final_readiness_timeout = startup_phase_timeout(
        startup_deadline,
        GATEWAY_STARTUP_PHASE_TIMEOUT,
        "gateway final readiness",
    )?;
    let readiness = wait_for_readiness_while_serving(
        servers.as_mut(),
        wait_for_gateway(&state, &runtime_shutdown, final_readiness_timeout),
    )
    .await;
    if let Err(error) = readiness {
        runtime_shutdown.cancel();
        state
            .shutdown_background_tasks(Duration::from_secs(10))
            .await;
        state
            .runtime_health
            .wait_stopped(Duration::from_secs(5))
            .await;
        state
            .tunnel
            .supervisors
            .shutdown_all(Duration::from_secs(10))
            .await;
        stop_auth_bridge(auth_bridge).await;
        if let Some(path) = &readiness_marker {
            let _ = tokio::fs::remove_file(path).await;
        }
        stop_runtime_logging(&state).await;
        checkpoint_storage_for_shutdown(&state).await;
        return Err(error);
    }
    if let Err(error) = state.runtime_health.mark_session_ready(&state).await {
        runtime_shutdown.cancel();
        state
            .shutdown_background_tasks(Duration::from_secs(10))
            .await;
        state
            .runtime_health
            .wait_stopped(Duration::from_secs(5))
            .await;
        state
            .tunnel
            .supervisors
            .shutdown_all(Duration::from_secs(10))
            .await;
        stop_auth_bridge(auth_bridge).await;
        if let Some(path) = &readiness_marker {
            let _ = tokio::fs::remove_file(path).await;
        }
        stop_runtime_logging(&state).await;
        checkpoint_storage_for_shutdown(&state).await;
        return Err(error);
    }
    if let Some(path) = &readiness_marker {
        tokio::fs::write(path, b"ready\n").await?;
    }
    state.spawn_background("startup-memory-trim", async move {
        let _ = boot_sync_completed.await;
        memory::trim_allocated_memory_after(Duration::from_secs(5)).await;
    });
    if let Some(ready) = ready {
        let _ = ready.send(());
    }
    let result = servers.await;
    runtime_shutdown.cancel();
    state
        .shutdown_background_tasks(Duration::from_secs(10))
        .await;
    state
        .runtime_health
        .wait_stopped(Duration::from_secs(5))
        .await;
    state
        .tunnel
        .supervisors
        .shutdown_all(Duration::from_secs(10))
        .await;
    stop_auth_bridge(auth_bridge).await;
    if let Some(path) = &readiness_marker {
        let _ = tokio::fs::remove_file(path).await;
    }
    stop_runtime_logging(&state).await;
    checkpoint_storage_for_shutdown(&state).await;
    result
}

async fn checkpoint_storage_for_shutdown(state: &AppState) {
    if let Err(error) = state.storage.store.checkpoint_for_shutdown().await {
        tracing::warn!(%error, "failed to checkpoint SQLite before shutdown");
    }
}

async fn stop_runtime_logging(state: &AppState) {
    if !state
        .runtime_health
        .shutdown_operational_log(Duration::from_secs(5))
        .await
    {
        tracing::warn!("diagnostic log writer did not stop within the shutdown deadline");
    }
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
    let wait = async {
        loop {
            let process_ready = state
                .gateway
                .client
                .health_serving(crate::go_backend::GATEWAY_HEALTH_PROCESS)
                .await
                .unwrap_or(false);
            if process_ready {
                match state.gateway.client.verify_bundle_compatibility().await {
                    Ok(_) => return Ok(()),
                    Err(crate::go_backend::BundleCompatibilityError::Unavailable(error)) => {
                        tracing::debug!(%error, "gateway compatibility probe is temporarily unavailable");
                    }
                    Err(error @ crate::go_backend::BundleCompatibilityError::Incompatible(_)) => {
                        return Err(error.into());
                    }
                }
            }
            tokio::select! {
                _ = shutdown.cancelled() => anyhow::bail!("startup cancelled"),
                _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            }
        }
    };
    match tokio::time::timeout(timeout, wait).await {
        Ok(result) => result,
        Err(_) => anyhow::bail!(
            "Go gateway control plane did not become ready within {:.1} seconds",
            timeout.as_secs_f64()
        ),
    }
}

async fn wait_for_gateway(
    state: &AppState,
    shutdown: &CancellationToken,
    timeout: Duration,
) -> anyhow::Result<()> {
    let wait = async {
        loop {
            let process_ready = state
                .gateway
                .client
                .health_serving(crate::go_backend::GATEWAY_HEALTH_PROCESS)
                .await
                .unwrap_or(false);
            let dataplane_ready = state
                .gateway
                .client
                .health_serving(crate::go_backend::GATEWAY_HEALTH_DATAPLANE)
                .await
                .unwrap_or(false);
            let auth_bridge_ready = state
                .gateway
                .client
                .health_serving(crate::go_backend::GATEWAY_HEALTH_AUTH_BRIDGE)
                .await
                .unwrap_or(false);
            let bundle_ready = if process_ready {
                match state.gateway.client.verify_bundle_compatibility().await {
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
            if bundle_ready && dataplane_ready && auth_bridge_ready && state.gateway_config_synced()
            {
                return Ok(());
            }
            tokio::select! {
                _ = shutdown.cancelled() => anyhow::bail!("startup cancelled"),
                _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            }
        }
    };
    match tokio::time::timeout(timeout, wait).await {
        Ok(result) => result,
        Err(_) => anyhow::bail!(
            "Go gateway did not become ready within {:.1} seconds",
            timeout.as_secs_f64()
        ),
    }
}

fn startup_phase_timeout(
    deadline: tokio::time::Instant,
    phase_cap: Duration,
    phase: &str,
) -> anyhow::Result<Duration> {
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        anyhow::bail!("application startup deadline exhausted before {phase}");
    }
    Ok(remaining.min(phase_cap))
}

fn application_startup_timeout() -> Duration {
    let supervisor_timeout = env::var("FN_KNOCK_SYNOLOGY_START_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs);
    application_startup_timeout_for_supervisor(supervisor_timeout)
}

fn application_startup_timeout_for_supervisor(supervisor_timeout: Option<Duration>) -> Duration {
    supervisor_timeout
        .map(|timeout| {
            timeout
                .saturating_sub(SYNOLOGY_SUPERVISOR_SHUTDOWN_MARGIN)
                .max(Duration::from_secs(1))
        })
        .unwrap_or(DEFAULT_APPLICATION_STARTUP_TIMEOUT)
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

    #[test]
    fn startup_phase_uses_the_smaller_of_remaining_budget_and_phase_cap() {
        let timeout = startup_phase_timeout(
            tokio::time::Instant::now() + Duration::from_secs(120),
            Duration::from_secs(60),
            "test phase",
        )
        .unwrap();

        assert!(timeout <= Duration::from_secs(60));
        assert!(timeout > Duration::from_secs(59));
    }

    #[test]
    fn startup_phase_fails_after_the_shared_deadline() {
        let error = startup_phase_timeout(
            tokio::time::Instant::now(),
            Duration::from_secs(60),
            "test phase",
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "application startup deadline exhausted before test phase"
        );
    }

    #[test]
    fn synology_application_budget_leaves_supervisor_cleanup_margin() {
        assert_eq!(
            application_startup_timeout_for_supervisor(Some(Duration::from_secs(180))),
            Duration::from_secs(150)
        );
        assert_eq!(
            application_startup_timeout_for_supervisor(Some(Duration::from_secs(20))),
            Duration::from_secs(1)
        );
        assert_eq!(
            application_startup_timeout_for_supervisor(None),
            DEFAULT_APPLICATION_STARTUP_TIMEOUT
        );
    }
}
