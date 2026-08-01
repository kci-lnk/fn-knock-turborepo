//! Windows SCM host for the fn-knock Rust control plane and Go data plane.
//!
//! The installer invokes `fn-knock-service.exe install`; normal SCM launches
//! have no arguments and enter the dispatcher. Runtime data is intentionally
//! kept outside Program Files under `%ProgramData%\FnKnock`.

use std::{
    env,
    ffi::OsString,
    fs,
    io::Write,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
    ptr,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, anyhow};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    net::TcpStream,
    process::Command,
    sync::oneshot,
};
use tokio_util::sync::CancellationToken;
use windows_service::{
    define_windows_service,
    service::{
        Service, ServiceAccess, ServiceAction, ServiceActionType, ServiceControl,
        ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceFailureActions,
        ServiceFailureResetPeriod, ServiceInfo, ServiceSidType, ServiceStartType, ServiceState,
        ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
    },
    UI::Shell::{FOLDERID_ProgramData, FOLDERID_System, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

use crate::{
    app,
    app_version::APP_LOCAL_VERSION,
    go_backend::{
        BundleCompatibilityError, GATEWAY_CONTROL_API_VERSION, GATEWAY_HEALTH_DATAPLANE,
        GATEWAY_HEALTH_PROCESS, GoBackendClient,
    },
    runtime_health::RotatingFile,
    settings::Settings,
};

const SERVICE_NAME: &str = "FnKnock";
const SERVICE_DISPLAY_NAME: &str = "fn-knock Gateway";
const SERVICE_ACCOUNT: &str = r"NT SERVICE\FnKnock";
const FIREWALL_RULE_NAME: &str = "FnKnock Gateway";
const SCM_RESTART_DELAYS_SECONDS: [u64; 3] = [5, 30, 60];
const ROOT_ACL_GRANTS: &[&str] = &[
    "*S-1-5-18:F",
    "*S-1-5-18:(OI)(CI)F",
    "*S-1-5-32-544:F",
    "*S-1-5-32-544:(OI)(CI)F",
    r"NT SERVICE\FnKnock:M",
    r"NT SERVICE\FnKnock:(OI)(CI)M",
];
const STATE_ACL_GRANTS: &[&str] = &[
    "*S-1-5-18:F",
    "*S-1-5-18:(OI)(CI)F",
    "*S-1-5-32-544:F",
    "*S-1-5-32-544:(OI)(CI)F",
    r"NT SERVICE\FnKnock:M",
    r"NT SERVICE\FnKnock:(OI)(CI)M",
    "*S-1-5-32-545:RX",
    "*S-1-5-32-545:(OI)(CI)RX",
];
const ROLLBACK_ACL_GRANTS: &[&str] = &[
    "*S-1-5-18:F",
    "*S-1-5-18:(OI)(CI)F",
    "*S-1-5-32-544:F",
    "*S-1-5-32-544:(OI)(CI)F",
];
const STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const START_PENDING_UPDATE_INTERVAL: Duration = Duration::from_secs(10);
const START_PENDING_WAIT_HINT: Duration = Duration::from_secs(30);
const LISTENER_SCOPE_APPLY_TIMEOUT: Duration = Duration::from_secs(10);
const LISTENER_SCOPE_STABILIZATION: Duration = Duration::from_secs(1);
const LISTENER_SCOPE_STABLE_PROBES: u8 = 3;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const GATEWAY_CONSOLE_LOG_FILE: &str = "gateway-console.log";
const GATEWAY_CONSOLE_LOG_MAX_BYTES: u64 = 512 * 1024;
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

pub fn command_main() -> anyhow::Result<()> {
    match env::args().nth(1).as_deref() {
        None | Some("run") | Some("--service") => {
            service_dispatcher::start(SERVICE_NAME, ffi_service_main)
                .context("start FnKnock SCM dispatcher")
        }
        Some("install") => install_service(),
        Some("uninstall") => uninstall_service(),
        Some("start") => start_service(),
        Some("stop") => stop_service(),
        Some("reset-panel-password") => reset_panel_password(),
        Some("print-paths") => {
            let paths = WindowsPaths::discover()?;
            println!("program_data={}", paths.root.display());
            println!("runtime_config={}", paths.runtime_config.display());
            println!("status={}", paths.status.display());
            Ok(())
        }
        Some("-h" | "--help" | "help") => {
            print_command_help();
            Ok(())
        }
        Some(command) => anyhow::bail!(
            "unknown command {command:?}; run fn-knock-service.exe --help for available commands"
        ),
    }
}

fn print_command_help() {
    println!("fn-knock Windows service host");
    println!();
    println!("Usage: fn-knock-service.exe <command>");
    println!();
    println!("Commands:");
    println!("  install                 Install or update the FnKnock SCM service");
    println!("  uninstall               Remove the FnKnock SCM service");
    println!("  start                   Start the FnKnock SCM service");
    println!("  stop                    Stop the FnKnock SCM service");
    println!("  reset-panel-password    Clear panel password, sessions, and login backoff");
    println!("  print-paths             Print ProgramData runtime paths");
    println!("  run | --service         Enter the SCM dispatcher (SCM use only)");
}

fn reset_panel_password() -> anyhow::Result<()> {
    let paths = WindowsPaths::discover()?;
    paths.create_runtime_directories()?;
    configure_program_data_environment(&paths);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build admin panel password reset runtime")?;
    runtime.block_on(app::reset_panel_password_command())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(error) = run_service() {
        let _ = write_emergency_status(&format!("service bootstrap failed: {error:#}"));
        // Returning normally from the generated service entrypoint can make a
        // bootstrap/runtime I/O failure look like a clean stop. Exit non-zero
        // so SCM applies the configured crash-recovery policy.
        std::process::exit(1);
    }
}

fn run_service() -> anyhow::Result<()> {
    app::init_tracing();
    let shutdown = CancellationToken::new();
    let handler_shutdown = shutdown.clone();
    let event_handler = move |event| match event {
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        ServiceControl::Stop | ServiceControl::Shutdown => {
            handler_shutdown.cancel();
            ServiceControlHandlerResult::NoError
        }
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .context("register FnKnock service control handler")?;
    report_status(
        &status_handle,
        ServiceState::StartPending,
        ServiceControlAccept::empty(),
        ServiceExitCode::NO_ERROR,
        1,
        START_PENDING_WAIT_HINT,
    )?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("build FnKnock service runtime")?;
    let outcome = runtime.block_on(supervise(&status_handle, shutdown));
    drop(runtime);

    match outcome {
        Ok(SupervisionOutcome::Stopped) => {
            report_status(
                &status_handle,
                ServiceState::Stopped,
                ServiceControlAccept::empty(),
                ServiceExitCode::NO_ERROR,
                0,
                Duration::ZERO,
            )?;
            Ok(())
        }
        Ok(SupervisionOutcome::DeterministicFailure(error)) => {
            tracing::error!(%error, "FnKnock service stopped after deterministic startup failure");
            let _ = ensure_faulted_status(&error.to_string());
            report_status(
                &status_handle,
                ServiceState::Stopped,
                ServiceControlAccept::empty(),
                ServiceExitCode::ServiceSpecific(1),
                0,
                Duration::ZERO,
            )?;
            Ok(())
        }
        Ok(SupervisionOutcome::UnexpectedFailure(error)) | Err(error) => {
            tracing::error!(%error, "FnKnock runtime group failed unexpectedly");
            // Do not report SERVICE_STOPPED: SCM treats the process exit as a crash and
            // applies the configured 5/30/60 second recovery policy.
            std::process::exit(1);
        }
    }
}

async fn supervise(
    status_handle: &ServiceStatusHandle,
    shutdown: CancellationToken,
) -> anyhow::Result<SupervisionOutcome> {
    let mut startup_status = StartPendingReporter::start(*status_handle);
    let paths = WindowsPaths::discover()?;
    paths.create_runtime_directories()?;
    if let Err(error) = enforce_service_account() {
        let _ = write_status(
            &paths,
            &ServiceStateFile::faulted(
                None,
                None,
                &format!("invalid Windows service identity: {error}"),
            ),
        );
        return Ok(SupervisionOutcome::DeterministicFailure(error));
    }

    let runtime_config = match load_runtime_config(&paths.runtime_config) {
        Ok(config) => config,
        Err(error) => {
            let _ = write_status(
                &paths,
                &ServiceStateFile::faulted(None, None, &format!("invalid runtime config: {error}")),
            );
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
    };
    let internal_token = match load_or_create_secret(&paths.secrets.join("internal-rpc-token")) {
        Ok(secret) => secret,
        Err(error) => {
            let _ = write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    None,
                    &format!("invalid internal RPC secret: {error}"),
                ),
            );
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
    };
    let hmac_secret = match load_or_create_secret(&paths.secrets.join("hmac-secret")) {
        Ok(secret) => secret,
        Err(error) => {
            let _ = write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    None,
                    &format!("invalid HMAC secret: {error}"),
                ),
            );
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
    };
    let altcha_hmac_key = match load_or_create_secret(&paths.secrets.join("altcha-hmac-key")) {
        Ok(secret) => secret,
        Err(error) => {
            let _ = write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    None,
                    &format!("invalid ALTCHA HMAC secret: {error}"),
                ),
            );
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
    };
    if let Err(error) = validate_installed_bundle() {
        let _ = write_status(
            &paths,
            &ServiceStateFile::faulted(
                Some(&runtime_config),
                None,
                &format!("bundle validation failed: {error}"),
            ),
        );
        return Ok(SupervisionOutcome::DeterministicFailure(error));
    }
    if let Err(error) = preflight_ports(&runtime_config) {
        let _ = write_status(
            &paths,
            &ServiceStateFile::faulted(
                Some(&runtime_config),
                None,
                &format!("port preflight failed: {error}"),
            ),
        );
        return Ok(SupervisionOutcome::DeterministicFailure(error));
    }
    configure_runtime_environment(
        &paths,
        &runtime_config,
        &internal_token,
        &hmac_secret,
        &altcha_hmac_key,
    )?;
    let settings = Settings::from_env();
    let go_client = match GoBackendClient::new(
        settings.go_backend_grpc_addr.clone(),
        settings.internal_rpc_token.clone(),
        settings.request_timeout,
    ) {
        Ok(client) => client,
        Err(error) => {
            let _ = write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    None,
                    &format!("invalid internal gRPC configuration: {error}"),
                ),
            );
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
    };

    write_status(
        &paths,
        &ServiceStateFile::starting(&runtime_config, None, "starting gateway"),
    )?;
    let job = JobObject::new()?;
    let mut gateway = spawn_gateway(&paths, &runtime_config, &internal_token)?;
    let gateway_pid = gateway.id();
    append_supervisor_diagnostic(
        &paths,
        "INFO",
        "gateway_process",
        "started",
        "windows_service_start",
        None,
    );
    if let Err(error) = job.assign(&gateway) {
        let _ = gateway.start_kill();
        let _ = gateway.wait().await;
        write_status(
            &paths,
            &ServiceStateFile::faulted(
                Some(&runtime_config),
                gateway_pid,
                &format!("gateway supervision setup failed: {error}"),
            ),
        )?;
        return Ok(SupervisionOutcome::UnexpectedFailure(error));
    }
    write_status(
        &paths,
        &ServiceStateFile::starting(&runtime_config, gateway_pid, "waiting for gateway"),
    )?;

    match wait_for_gateway_control_plane(&go_client, &mut gateway, &shutdown).await {
        Ok(()) => {}
        Err(GatewayStartupFailure::Cancelled) => {
            startup_status.stop();
            report_status(
                status_handle,
                ServiceState::StopPending,
                ServiceControlAccept::empty(),
                ServiceExitCode::NO_ERROR,
                1,
                Duration::from_secs(20),
            )?;
            shutdown_gateway_only(&go_client, &mut gateway).await;
            write_status(
                &paths,
                &ServiceStateFile::stopped(&runtime_config, gateway_pid),
            )?;
            drop(job);
            return Ok(SupervisionOutcome::Stopped);
        }
        Err(GatewayStartupFailure::Deterministic(error)) => {
            let _ = gateway.start_kill();
            let _ = gateway.wait().await;
            write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &format!("gateway compatibility failed: {error}"),
                ),
            )?;
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
        Err(GatewayStartupFailure::Unexpected(error)) => {
            write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &gateway_failure_message("gateway startup failed unexpectedly", &error, &paths),
                ),
            )?;
            return Ok(SupervisionOutcome::UnexpectedFailure(error));
        }
    }
    match set_gateway_listener_scope(&go_client, &runtime_config).await {
        Ok(()) => {}
        Err(GatewayStartupFailure::Deterministic(error)) => {
            let _ = gateway.start_kill();
            let _ = gateway.wait().await;
            write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &format!("gateway listener configuration failed: {error}"),
                ),
            )?;
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
        Err(GatewayStartupFailure::Unexpected(error)) => {
            write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &gateway_failure_message(
                        "gateway listener configuration failed unexpectedly",
                        &error,
                        &paths,
                    ),
                ),
            )?;
            return Ok(SupervisionOutcome::UnexpectedFailure(error));
        }
        Err(GatewayStartupFailure::Cancelled) => {
            unreachable!("setting gateway listener scope does not wait for service cancellation")
        }
    }
    if shutdown.is_cancelled() {
        startup_status.stop();
        report_status(
            status_handle,
            ServiceState::StopPending,
            ServiceControlAccept::empty(),
            ServiceExitCode::NO_ERROR,
            1,
            Duration::from_secs(20),
        )?;
        shutdown_gateway_only(&go_client, &mut gateway).await;
        write_status(
            &paths,
            &ServiceStateFile::stopped(&runtime_config, gateway_pid),
        )?;
        drop(job);
        return Ok(SupervisionOutcome::Stopped);
    }

    let (ready_tx, mut ready_rx) = oneshot::channel();
    let app_future = app::run_with_settings(settings, shutdown.clone(), Some(ready_tx));
    tokio::pin!(app_future);

    let startup_result = tokio::select! {
        biased;
        _ = shutdown.cancelled() => {
            startup_status.stop();
            report_status(
                status_handle,
                ServiceState::StopPending,
                ServiceControlAccept::empty(),
                ServiceExitCode::NO_ERROR,
                1,
                Duration::from_secs(20),
            )?;
            graceful_shutdown(&go_client, &mut gateway, &mut app_future, &paths, &runtime_config, gateway_pid).await;
            drop(job);
            return Ok(SupervisionOutcome::Stopped);
        }
        result = &mut app_future => {
            Err(
                result.err().unwrap_or_else(|| anyhow!("Rust admin runtime stopped during startup"))
            )
        }
        ready = &mut ready_rx => ready.context("Rust admin runtime exited before readiness"),
        status = gateway.wait() => {
            append_supervisor_diagnostic(
                &paths,
                "ERROR",
                "gateway_process",
                "exited",
                "startup_exit",
                status.as_ref().ok().and_then(|value| value.code()),
            );
            let error = anyhow!(
                "Go gateway exited during startup: {}",
                display_exit_status(status)
            );
            shutdown.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(5), &mut app_future).await;
            let _ = write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &gateway_failure_message("gateway exited during startup", &error, &paths),
                ),
            );
            return Ok(SupervisionOutcome::UnexpectedFailure(error));
        }
        _ = tokio::time::sleep(STARTUP_TIMEOUT) => {
            let error = anyhow!("Rust/Go runtime group did not become ready within 60 seconds");
            shutdown.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(5), &mut app_future).await;
            shutdown_gateway_only(&go_client, &mut gateway).await;
            let _ = write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &error.to_string(),
                ),
            );
            return Ok(SupervisionOutcome::UnexpectedFailure(error));
        }
    };
    if let Err(error) = startup_result {
        shutdown.cancel();
        let _ = gateway.start_kill();
        let _ = gateway.wait().await;
        write_status(
            &paths,
            &ServiceStateFile::faulted(Some(&runtime_config), gateway_pid, &error.to_string()),
        )?;
        if startup_error_is_deterministic(&error) {
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
        return Ok(SupervisionOutcome::UnexpectedFailure(error));
    }

    let listener_result = tokio::select! {
        biased;
        _ = shutdown.cancelled() => Err(GatewayStartupFailure::Cancelled),
        result = &mut app_future => Err(GatewayStartupFailure::Unexpected(
            result.err().unwrap_or_else(|| anyhow!("Rust admin runtime stopped while verifying gateway listener readiness"))
        )),
        result = wait_for_gateway_listener_scope(&go_client, &mut gateway, &runtime_config, &shutdown) => result,
    };
    match listener_result {
        Ok(()) => {}
        Err(GatewayStartupFailure::Cancelled) => {
            startup_status.stop();
            report_status(
                status_handle,
                ServiceState::StopPending,
                ServiceControlAccept::empty(),
                ServiceExitCode::NO_ERROR,
                1,
                Duration::from_secs(20),
            )?;
            graceful_shutdown(
                &go_client,
                &mut gateway,
                &mut app_future,
                &paths,
                &runtime_config,
                gateway_pid,
            )
            .await;
            drop(job);
            return Ok(SupervisionOutcome::Stopped);
        }
        Err(GatewayStartupFailure::Deterministic(error)) => {
            shutdown.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(5), &mut app_future).await;
            let _ = gateway.start_kill();
            let _ = gateway.wait().await;
            write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &format!("gateway listener readiness failed: {error}"),
                ),
            )?;
            return Ok(SupervisionOutcome::DeterministicFailure(error));
        }
        Err(GatewayStartupFailure::Unexpected(error)) => {
            shutdown.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(5), &mut app_future).await;
            let _ = gateway.start_kill();
            let _ = gateway.wait().await;
            write_status(
                &paths,
                &ServiceStateFile::faulted(
                    Some(&runtime_config),
                    gateway_pid,
                    &gateway_failure_message(
                        "gateway listener readiness failed unexpectedly",
                        &error,
                        &paths,
                    ),
                ),
            )?;
            return Ok(SupervisionOutcome::UnexpectedFailure(error));
        }
    }

    startup_status.stop();
    report_status(
        status_handle,
        ServiceState::Running,
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        ServiceExitCode::NO_ERROR,
        0,
        Duration::ZERO,
    )?;
    write_status(
        &paths,
        &ServiceStateFile::running(&runtime_config, gateway_pid),
    )?;
    append_supervisor_diagnostic(
        &paths,
        "INFO",
        "management",
        "started",
        "windows_service_running",
        None,
    );

    let outcome = tokio::select! {
        _ = shutdown.cancelled() => {
            append_supervisor_diagnostic(&paths, "INFO", "supervisor", "stop_requested", "scm_stop", None);
            report_status(
                status_handle,
                ServiceState::StopPending,
                ServiceControlAccept::empty(),
                ServiceExitCode::NO_ERROR,
                1,
                Duration::from_secs(20),
            )?;
            graceful_shutdown(&go_client, &mut gateway, &mut app_future, &paths, &runtime_config, gateway_pid).await;
            SupervisionOutcome::Stopped
        }
        result = &mut app_future => {
            let error = result.err().unwrap_or_else(|| anyhow!("Rust admin runtime exited unexpectedly"));
            append_supervisor_diagnostic(&paths, "ERROR", "management", "exited", "unexpected_exit", None);
            shutdown.cancel();
            let _ = go_client.request_shutdown().await;
            let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, gateway.wait()).await;
            SupervisionOutcome::UnexpectedFailure(error)
        }
        status = gateway.wait() => {
            append_supervisor_diagnostic(
                &paths,
                "ERROR",
                "gateway_process",
                "exited",
                "unexpected_exit",
                status.as_ref().ok().and_then(|value| value.code()),
            );
            shutdown.cancel();
            let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut app_future).await;
            SupervisionOutcome::UnexpectedFailure(anyhow!(
                "Go gateway exited unexpectedly: {}",
                display_exit_status(status)
            ))
        }
    };
    if let SupervisionOutcome::UnexpectedFailure(error) = &outcome {
        let _ = write_status(
            &paths,
            &ServiceStateFile::faulted(
                Some(&runtime_config),
                gateway_pid,
                &gateway_failure_message("runtime group failed", error, &paths),
            ),
        );
    }
    drop(job);
    Ok(outcome)
}

async fn shutdown_gateway_only(go_client: &GoBackendClient, gateway: &mut tokio::process::Child) {
    let _ = tokio::time::timeout(Duration::from_secs(2), go_client.request_shutdown()).await;
    if tokio::time::timeout(SHUTDOWN_TIMEOUT, gateway.wait())
        .await
        .is_err()
    {
        let _ = gateway.start_kill();
        let _ = gateway.wait().await;
    }
}

async fn graceful_shutdown<F>(
    go_client: &GoBackendClient,
    gateway: &mut tokio::process::Child,
    app_future: &mut std::pin::Pin<&mut F>,
    paths: &WindowsPaths,
    runtime_config: &WindowsRuntimeConfig,
    gateway_pid: Option<u32>,
) where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    let _ = write_status(
        paths,
        &ServiceStateFile::stopping(runtime_config, gateway_pid),
    );
    let _ = tokio::time::timeout(Duration::from_secs(2), go_client.request_shutdown()).await;
    let graceful = async {
        let _ = app_future.await;
        let _ = gateway.wait().await;
    };
    if tokio::time::timeout(SHUTDOWN_TIMEOUT, graceful)
        .await
        .is_err()
    {
        let _ = gateway.start_kill();
        let _ = gateway.wait().await;
    }
    let _ = write_status(
        paths,
        &ServiceStateFile::stopped(runtime_config, gateway_pid),
    );
}

async fn wait_for_gateway_control_plane(
    client: &GoBackendClient,
    gateway: &mut tokio::process::Child,
    shutdown: &CancellationToken,
) -> Result<(), GatewayStartupFailure> {
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
    loop {
        let gateway_status = gateway.try_wait().map_err(|error| {
            GatewayStartupFailure::Unexpected(anyhow!(
                "query Go gateway status during startup: {error}"
            ))
        })?;
        if let Some(status) = gateway_status {
            return Err(GatewayStartupFailure::Unexpected(anyhow!(
                "Go gateway exited before readiness: {status}"
            )));
        }
        let process = client
            .health_serving(GATEWAY_HEALTH_PROCESS)
            .await
            .unwrap_or(false);
        let compatible = if process {
            match client.verify_bundle_compatibility().await {
                Ok(_) => true,
                Err(BundleCompatibilityError::Unavailable(error)) => {
                    tracing::debug!(%error, "gateway compatibility probe is temporarily unavailable");
                    false
                }
                Err(error @ BundleCompatibilityError::Incompatible(_)) => {
                    return Err(GatewayStartupFailure::Deterministic(error.into()));
                }
            }
        } else {
            false
        };
        if compatible {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(GatewayStartupFailure::Unexpected(anyhow!(
                "Go gateway control plane did not become ready within 60 seconds"
            )));
        }
        tokio::select! {
            _ = shutdown.cancelled() => return Err(GatewayStartupFailure::Cancelled),
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
    }
}

async fn set_gateway_listener_scope(
    client: &GoBackendClient,
    config: &WindowsRuntimeConfig,
) -> Result<(), GatewayStartupFailure> {
    let applied_scope = client
        .set_gateway_listener_scope(&config.listener_scope)
        .await
        .map_err(|error| {
            GatewayStartupFailure::Unexpected(anyhow!(
                "apply persisted gateway listener scope: {error}"
            ))
        })?;
    if applied_scope != config.listener_scope {
        return Err(GatewayStartupFailure::Deterministic(anyhow!(
            "gateway returned listener scope {applied_scope:?}, expected {:?}",
            config.listener_scope
        )));
    }
    Ok(())
}

async fn wait_for_gateway_listener_scope(
    client: &GoBackendClient,
    gateway: &mut tokio::process::Child,
    config: &WindowsRuntimeConfig,
    shutdown: &CancellationToken,
) -> Result<(), GatewayStartupFailure> {
    // SetGatewayListenerConfig queues a listener rebind in the current gateway
    // protocol. The Rust auth bridge must be started before the Go data plane
    // can become healthy, so this stability check deliberately runs only after
    // the Rust runtime has reported complete readiness.
    let applied_at = tokio::time::Instant::now();
    let deadline = applied_at + LISTENER_SCOPE_APPLY_TIMEOUT;
    let proxy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), config.proxy_port);
    let mut stable_probes = 0_u8;
    loop {
        let gateway_status = gateway.try_wait().map_err(|error| {
            GatewayStartupFailure::Unexpected(anyhow!(
                "query Go gateway status during listener rebind: {error}"
            ))
        })?;
        if let Some(status) = gateway_status {
            return Err(GatewayStartupFailure::Unexpected(anyhow!(
                "Go gateway exited while applying listener scope: {status}"
            )));
        }

        let configured_scope = client.get_gateway_listener_scope().await.ok();
        let dataplane = client
            .health_serving(GATEWAY_HEALTH_DATAPLANE)
            .await
            .unwrap_or(false);
        let proxy_accepting = matches!(
            tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(proxy_addr)).await,
            Ok(Ok(_))
        );
        if configured_scope.as_deref() == Some(config.listener_scope.as_str())
            && dataplane
            && proxy_accepting
        {
            stable_probes = stable_probes.saturating_add(1);
            if stable_probes >= LISTENER_SCOPE_STABLE_PROBES
                && applied_at.elapsed() >= LISTENER_SCOPE_STABILIZATION
            {
                return Ok(());
            }
        } else {
            stable_probes = 0;
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(GatewayStartupFailure::Unexpected(anyhow!(
                "gateway listener scope {:?} did not reach a stable serving state within {} seconds",
                config.listener_scope,
                LISTENER_SCOPE_APPLY_TIMEOUT.as_secs()
            )));
        }
        tokio::select! {
            _ = shutdown.cancelled() => return Err(GatewayStartupFailure::Cancelled),
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
    }
}

fn spawn_gateway(
    paths: &WindowsPaths,
    config: &WindowsRuntimeConfig,
    token: &str,
) -> anyhow::Result<tokio::process::Child> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let gateway = env::current_exe()
        .context("resolve service executable")?
        .parent()
        .context("service executable has no parent directory")?
        .join("fn-knock-gateway.exe");
    if !gateway.is_file() {
        anyhow::bail!("missing bundled gateway executable: {}", gateway.display());
    }
    let gateway_config = paths.gateway.join("config.json");
    let gateway_log_path = gateway_console_log_path(paths);
    let gateway_console = Arc::new(Mutex::new(
        RotatingFile::new(gateway_log_path.clone(), GATEWAY_CONSOLE_LOG_MAX_BYTES)
            .with_context(|| format!("open gateway log {}", gateway_log_path.display()))?,
    ));
    let mut command = Command::new(&gateway);
    command
        .arg("--admin-port")
        .arg(config.grpc_port.to_string())
        .arg("--proxy-port")
        .arg(config.proxy_port.to_string())
        .arg("-c")
        .arg(&gateway_config)
        .arg("--logs-dir")
        .arg(&paths.logs)
        .arg("--waf-dir")
        .arg(&paths.waf)
        .current_dir(&paths.gateway)
        .env("FN_KNOCK_INTERNAL_RPC_TOKEN", token)
        .env("FN_KNOCK_DATA_DIR", &paths.data)
        .env(
            "FN_KNOCK_DIAGNOSTIC_LOG_DIR",
            paths.data.join("runtime/logs"),
        )
        .env("GO_REPROXY_LOG", "1")
        .env("BACKEND_PORT", config.backend_port.to_string())
        .env("AUTH_PORT", config.auth_port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW);
    let mut child = command
        .spawn()
        .with_context(|| format!("start bundled gateway {}", gateway.display()))?;
    if let Some(stdout) = child.stdout.take() {
        spawn_gateway_console_pump(stdout, gateway_console.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_gateway_console_pump(stderr, gateway_console);
    }
    Ok(child)
}

fn spawn_gateway_console_pump<R>(mut reader: R, writer: Arc<Mutex<RotatingFile>>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            let read = match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            let Ok(mut writer) = writer.lock() else {
                break;
            };
            if writer.write(&buffer[..read]).is_err() {
                break;
            }
        }
        if let Ok(mut writer) = writer.lock() {
            let _ = writer.flush();
        }
    });
}

fn gateway_console_log_path(paths: &WindowsPaths) -> PathBuf {
    paths.logs.join(GATEWAY_CONSOLE_LOG_FILE)
}

fn append_supervisor_diagnostic(
    paths: &WindowsPaths,
    level: &str,
    component: &str,
    event: &str,
    reason_code: &str,
    exit_code: Option<i32>,
) {
    let directory = paths.data.join("runtime/logs");
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join("supervisor.jsonl");
    let record = serde_json::json!({
        "time": OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default(),
        "level": level,
        "component": component,
        "event": event,
        "reason_code": reason_code,
        "fields": { "exit_code": exit_code, "signal": serde_json::Value::Null },
    });
    let Ok(mut bytes) = serde_json::to_vec(&record) else {
        return;
    };
    bytes.push(b'\n');
    if let Ok(mut file) = RotatingFile::new(path, GATEWAY_CONSOLE_LOG_MAX_BYTES) {
        let _ = file.write(&bytes);
        let _ = file.flush();
    }
    let hints = paths.data.join("runtime/supervisor-events");
    let hint = hints.join(format!(
        "{}-{}-{}.json",
        OffsetDateTime::now_utc().unix_timestamp_nanos(),
        std::process::id(),
        event
    ));
    let _ = atomic_write(&hint, &bytes);
    cleanup_supervisor_hints(&hints);
}

fn cleanup_supervisor_hints(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let hint_max_age = Duration::from_secs(7 * 24 * 60 * 60);
    let temp_max_age = Duration::from_secs(24 * 60 * 60);
    let mut retained_hints = Vec::new();
    let mut retained_temps = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let (max_age, retained) = match path.extension().and_then(|value| value.to_str()) {
            Some("json") => (hint_max_age, &mut retained_hints),
            Some("tmp") => (temp_max_age, &mut retained_temps),
            _ => continue,
        };
        if !entry.metadata().is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        let expired = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > max_age);
        if expired {
            let _ = fs::remove_file(path);
        } else {
            retained.push(path);
            if retained.len() > 64 {
                retained.sort();
                let excess = retained.len().saturating_sub(32);
                for path in retained.drain(..excess) {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
    for retained in [&mut retained_hints, &mut retained_temps] {
        retained.sort();
        let excess = retained.len().saturating_sub(32);
        for path in retained.drain(..excess) {
            let _ = fs::remove_file(path);
        }
    }
}

fn gateway_failure_message(prefix: &str, error: &anyhow::Error, paths: &WindowsPaths) -> String {
    format!(
        "{prefix}: {error}; gateway diagnostics: {}",
        gateway_console_log_path(paths).display()
    )
}

#[derive(Debug)]
enum SupervisionOutcome {
    Stopped,
    DeterministicFailure(anyhow::Error),
    UnexpectedFailure(anyhow::Error),
}

#[derive(Debug)]
enum GatewayStartupFailure {
    Cancelled,
    Deterministic(anyhow::Error),
    Unexpected(anyhow::Error),
}

fn startup_error_is_deterministic(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<BundleCompatibilityError>(),
        Some(BundleCompatibilityError::Incompatible(_))
    )
}

/// Keeps SCM informed while startup crosses multiple asynchronous phases.
/// A dedicated thread lets Drop synchronously join the reporter before a
/// terminal status is published, avoiding a late StartPending overwrite.
struct StartPendingReporter {
    stop: Arc<(Mutex<bool>, Condvar)>,
    thread: Option<thread::JoinHandle<()>>,
}

impl StartPendingReporter {
    fn start(status_handle: ServiceStatusHandle) -> Self {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let thread_stop = stop.clone();
        let thread = thread::spawn(move || {
            let mut checkpoint = 2_u32;
            loop {
                let (lock, condition) = &*thread_stop;
                let stopped = lock.lock().unwrap_or_else(|error| error.into_inner());
                let (stopped, _) = condition
                    .wait_timeout(stopped, START_PENDING_UPDATE_INTERVAL)
                    .unwrap_or_else(|error| error.into_inner());
                if *stopped {
                    break;
                }
                drop(stopped);
                if let Err(error) = report_status(
                    &status_handle,
                    ServiceState::StartPending,
                    ServiceControlAccept::empty(),
                    ServiceExitCode::NO_ERROR,
                    checkpoint,
                    START_PENDING_WAIT_HINT,
                ) {
                    tracing::warn!(%error, "failed to refresh SCM startup progress");
                    break;
                }
                checkpoint = checkpoint.saturating_add(1);
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }

    fn stop(&mut self) {
        let (lock, condition) = &*self.stop;
        *lock.lock().unwrap_or_else(|error| error.into_inner()) = true;
        condition.notify_all();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("SCM startup progress reporter panicked");
        }
    }
}

impl Drop for StartPendingReporter {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone, Debug)]
struct WindowsPaths {
    root: PathBuf,
    config: PathBuf,
    runtime_config: PathBuf,
    gateway: PathBuf,
    data: PathBuf,
    logs: PathBuf,
    waf: PathBuf,
    certificates: PathBuf,
    secrets: PathBuf,
    state: PathBuf,
    status: PathBuf,
    rollback: PathBuf,
}

impl WindowsPaths {
    fn discover() -> anyhow::Result<Self> {
        let program_data = trusted_program_data_directory()?;
        let root = program_data.join("FnKnock");
        let config = root.join("config");
        let state = root.join("state");
        Ok(Self {
            runtime_config: config.join("runtime.json"),
            status: state.join("status.json"),
            gateway: root.join("gateway"),
            data: root.join("data"),
            logs: root.join("logs"),
            waf: root.join("waf"),
            certificates: root.join("certificates"),
            secrets: root.join("secrets"),
            rollback: root.join("rollback"),
            root,
            config,
            state,
        })
    }

    fn create_runtime_directories(&self) -> anyhow::Result<()> {
        for path in [
            &self.root,
            &self.config,
            &self.gateway,
            &self.data,
            &self.logs,
            &self.waf,
            &self.certificates,
            &self.secrets,
            &self.state,
            &self.rollback,
        ] {
            fs::create_dir_all(path)
                .with_context(|| format!("create runtime directory {}", path.display()))?;
        }
        Ok(())
    }
}

fn trusted_program_data_directory() -> anyhow::Result<PathBuf> {
    trusted_known_folder(&FOLDERID_ProgramData, "ProgramData")
}

fn trusted_system_directory() -> anyhow::Result<PathBuf> {
    trusted_known_folder(&FOLDERID_System, "System")
}

fn trusted_known_folder(
    folder_id: &windows_sys::core::GUID,
    label: &str,
) -> anyhow::Result<PathBuf> {
    let mut raw = ptr::null_mut();
    // SAFETY: SHGetKnownFolderPath initializes raw on success. The allocation
    // is owned by the COM task allocator and is released below.
    let result = unsafe {
        SHGetKnownFolderPath(folder_id, KF_FLAG_DEFAULT as u32, ptr::null_mut(), &mut raw)
    };
    if result < 0 || raw.is_null() {
        anyhow::bail!(
            "resolve Windows {label} known folder failed with HRESULT 0x{:08x}",
            result as u32
        );
    }
    let mut length = 0_usize;
    // SAFETY: a successful call returns a NUL-terminated UTF-16 string.
    unsafe {
        while *raw.add(length) != 0 {
            length += 1;
        }
    }
    // SAFETY: raw points to length initialized UTF-16 code units.
    let path = PathBuf::from(OsString::from_wide(unsafe {
        std::slice::from_raw_parts(raw, length)
    }));
    // SAFETY: raw was allocated by SHGetKnownFolderPath.
    unsafe { CoTaskMemFree(raw.cast()) };
    if !path.is_absolute() {
        anyhow::bail!("Windows {label} known folder is not absolute");
    }
    Ok(path)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct WindowsRuntimeConfig {
    schema_version: u32,
    onboarding_complete: bool,
    admin_port: u16,
    backend_port: u16,
    auth_port: u16,
    grpc_port: u16,
    proxy_port: u16,
    listener_scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledBundleIdentity {
    version: String,
    commit: String,
    gateway_commit: String,
    control_api_version: u64,
    target: String,
    files: Vec<String>,
}

fn validate_installed_bundle() -> anyhow::Result<()> {
    let install_dir = env::current_exe()?
        .parent()
        .context("service executable has no parent directory")?
        .to_path_buf();
    let manifest_path = install_dir.join("bundle.json");
    let bytes = fs::read(&manifest_path)
        .with_context(|| format!("read installed bundle identity {}", manifest_path.display()))?;
    let identity =
        serde_json::from_slice::<InstalledBundleIdentity>(&bytes).with_context(|| {
            format!(
                "parse installed bundle identity {}",
                manifest_path.display()
            )
        })?;
    if identity.version != APP_LOCAL_VERSION {
        anyhow::bail!(
            "bundle version mismatch: service={APP_LOCAL_VERSION}, manifest={}",
            identity.version
        );
    }
    let local_commit = option_env!("FN_KNOCK_COMMIT").unwrap_or("").trim();
    if local_commit.is_empty() || identity.commit.trim().is_empty() {
        anyhow::bail!("release bundle commit metadata is missing");
    }
    if identity.commit.trim() != local_commit {
        anyhow::bail!(
            "bundle commit mismatch: service={local_commit}, manifest={}",
            identity.commit.trim()
        );
    }
    let expected_gateway_commit = option_env!("FN_KNOCK_GATEWAY_COMMIT").unwrap_or("").trim();
    if expected_gateway_commit.is_empty() || identity.gateway_commit.trim().is_empty() {
        anyhow::bail!("release gateway source commit metadata is missing");
    }
    if identity.gateway_commit.trim() != expected_gateway_commit {
        anyhow::bail!(
            "gateway source commit mismatch: service={expected_gateway_commit}, manifest={}",
            identity.gateway_commit.trim()
        );
    }
    if identity.control_api_version != GATEWAY_CONTROL_API_VERSION {
        anyhow::bail!(
            "bundle control API mismatch: service={}, manifest={}",
            GATEWAY_CONTROL_API_VERSION,
            identity.control_api_version
        );
    }
    if identity.target != "windows-x86_64" {
        anyhow::bail!("unsupported bundle target {:?}", identity.target);
    }
    for required in [
        "fn-knock.exe",
        "fn-knock-service.exe",
        "fn-knock-gateway.exe",
        "ui/www",
        "server-auth-view/dist",
    ] {
        if !identity.files.iter().any(|entry| entry == required) {
            anyhow::bail!("bundle identity is missing required component {required}");
        }
        if !install_dir.join(required).exists() {
            anyhow::bail!("installed bundle component {required} does not exist");
        }
    }
    Ok(())
}

impl Default for WindowsRuntimeConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            onboarding_complete: false,
            admin_port: 7991,
            backend_port: 7998,
            auth_port: 7997,
            grpc_port: 7996,
            proxy_port: 7999,
            listener_scope: "all".to_string(),
        }
    }
}

impl WindowsRuntimeConfig {
    fn validate(&mut self) -> anyhow::Result<()> {
        if self.schema_version != 1 {
            anyhow::bail!("unsupported runtime config schema {}", self.schema_version);
        }
        self.listener_scope = self.listener_scope.trim().to_ascii_lowercase();
        if !matches!(self.listener_scope.as_str(), "loopback" | "all") {
            anyhow::bail!("listener_scope must be loopback or all");
        }
        let ports = [
            self.admin_port,
            self.backend_port,
            self.auth_port,
            self.grpc_port,
            self.proxy_port,
        ];
        if ports.contains(&0) {
            anyhow::bail!("runtime ports must be between 1 and 65535");
        }
        let mut distinct = ports.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() != ports.len() {
            anyhow::bail!("admin/backend/auth/grpc/proxy ports must be distinct");
        }
        Ok(())
    }
}

fn load_runtime_config(path: &Path) -> anyhow::Result<WindowsRuntimeConfig> {
    if !path.exists() {
        let config = WindowsRuntimeConfig::default();
        atomic_write_json(path, &config)?;
        return Ok(config);
    }
    let content = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut config = serde_json::from_slice::<WindowsRuntimeConfig>(&content)
        .with_context(|| format!("parse {}", path.display()))?;
    config.validate()?;
    // Windows gateway port 7999 is the public data-plane entry. Older desktop
    // releases defaulted it to loopback, which made the gateway unreachable
    // from other machines even when the firewall rule allowed the port.
    // Migrate persisted installations as well as using the corrected default
    // for new installs.
    if config.listener_scope == "loopback" {
        config.listener_scope = "all".to_string();
        atomic_write_json(path, &config)?;
    }
    Ok(config)
}

fn preflight_ports(config: &WindowsRuntimeConfig) -> anyhow::Result<()> {
    let mut listeners = Vec::new();
    for port in [
        config.admin_port,
        config.backend_port,
        config.auth_port,
        config.grpc_port,
    ] {
        for ip in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ] {
            let addr = SocketAddr::new(ip, port);
            listeners.push(bind_preflight_listener(addr)?);
        }
    }
    let proxy_ips = if config.listener_scope == "all" {
        [
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        ]
    } else {
        [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ]
    };
    for proxy_ip in proxy_ips {
        let proxy_addr = SocketAddr::new(proxy_ip, config.proxy_port);
        listeners.push(bind_preflight_listener(proxy_addr)?);
    }
    drop(listeners);
    Ok(())
}

fn bind_preflight_listener(addr: SocketAddr) -> anyhow::Result<TcpListener> {
    let domain = if addr.is_ipv6() {
        Domain::IPV6
    } else {
        Domain::IPV4
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .with_context(|| format!("create preflight socket for {addr}"))?;
    if addr.is_ipv6() {
        socket
            .set_only_v6(true)
            .with_context(|| format!("configure IPv6-only preflight socket for {addr}"))?;
    }
    socket
        .bind(&addr.into())
        .with_context(|| format!("port {addr} is unavailable"))?;
    socket
        .listen(1)
        .with_context(|| format!("listen on preflight port {addr}"))?;
    Ok(socket.into())
}

fn configure_runtime_environment(
    paths: &WindowsPaths,
    config: &WindowsRuntimeConfig,
    internal_token: &str,
    hmac_secret: &str,
    altcha_hmac_key: &str,
) -> anyhow::Result<()> {
    let install_dir = env::current_exe()?
        .parent()
        .context("service executable has no parent directory")?
        .to_path_buf();
    configure_program_data_environment(paths);
    set_env("BACKEND_HOST", "127.0.0.1");
    set_env("BACKEND_PORT", config.backend_port.to_string());
    set_env("AUTH_HOST", "127.0.0.1");
    set_env("AUTH_PORT", config.auth_port.to_string());
    set_env("ADMIN_VIEW_HOST", "127.0.0.1");
    set_env("ADMIN_VIEW_PORT", config.admin_port.to_string());
    set_env(
        "GO_BACKEND_GRPC_ADDR",
        format!("127.0.0.1:{}", config.grpc_port),
    );
    set_env("GO_REPROXY_PORT", config.proxy_port.to_string());
    set_env("FN_KNOCK_INTERNAL_RPC_TOKEN", internal_token);
    if env::var_os("HMAC_SECRET").is_none() {
        set_env("HMAC_SECRET", hmac_secret);
    }
    if env::var("ALTCHA_HMAC_KEY")
        .ok()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        set_env("ALTCHA_HMAC_KEY", altcha_hmac_key);
    }
    set_env("NODE_ENV", "production");
    set_env("EXPOSE_RUNTIME_HMAC_SECRET", "0");
    set_env("ADMIN_STATIC_PATH", install_dir.join("ui/www"));
    set_env(
        "AUTH_STATIC_PATH",
        install_dir.join("server-auth-view/dist"),
    );
    env::set_current_dir(&install_dir)
        .with_context(|| format!("set current directory to {}", install_dir.display()))?;
    Ok(())
}

fn configure_program_data_environment(paths: &WindowsPaths) {
    set_env("FN_KNOCK_RUNTIME_TARGET", "windows");
    set_env("FN_KNOCK_DATA_DIR", &paths.data);
    set_env("FN_KNOCK_GATEWAY_CONFIG_DIR", &paths.gateway);
    set_env("FN_KNOCK_WAF_DIR", &paths.waf);
    set_env(
        "FN_KNOCK_SQLITE_PATH",
        paths.data.join("storage/fn-knock.sqlite3"),
    );
}

fn set_env(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    // SAFETY: service environment is configured synchronously before any
    // application worker tasks are spawned.
    unsafe { env::set_var(key, value) }
}

fn load_or_create_secret(path: &Path) -> anyhow::Result<String> {
    if path.exists() {
        let secret = fs::read_to_string(path)
            .with_context(|| format!("read secret {}", path.display()))?
            .trim()
            .to_string();
        if secret.len() < 32 {
            anyhow::bail!("secret {} is invalid", path.display());
        }
        return Ok(secret);
    }
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let secret = hex::encode(bytes);
    atomic_write(path, secret.as_bytes())?;
    Ok(secret)
}

#[derive(Debug, Serialize)]
struct ServiceStateFile {
    schema_version: u32,
    state: &'static str,
    updated_at: String,
    service_pid: u32,
    gateway_pid: Option<u32>,
    onboarding_complete: bool,
    ports: Option<ServicePorts>,
    listener_scope: Option<String>,
    message: String,
}

#[derive(Debug, Serialize)]
struct ServicePorts {
    admin: u16,
    backend: u16,
    auth: u16,
    grpc: u16,
    proxy: u16,
}

impl ServiceStateFile {
    fn starting(config: &WindowsRuntimeConfig, gateway_pid: Option<u32>, message: &str) -> Self {
        Self::new("starting", Some(config), gateway_pid, message)
    }

    fn running(config: &WindowsRuntimeConfig, gateway_pid: Option<u32>) -> Self {
        Self::new("running", Some(config), gateway_pid, "ready")
    }

    fn stopping(config: &WindowsRuntimeConfig, gateway_pid: Option<u32>) -> Self {
        Self::new("stopping", Some(config), gateway_pid, "graceful shutdown")
    }

    fn stopped(config: &WindowsRuntimeConfig, gateway_pid: Option<u32>) -> Self {
        Self::new("stopped", Some(config), gateway_pid, "stopped")
    }

    fn faulted(
        config: Option<&WindowsRuntimeConfig>,
        gateway_pid: Option<u32>,
        message: &str,
    ) -> Self {
        Self::new("faulted", config, gateway_pid, message)
    }

    fn new(
        state: &'static str,
        config: Option<&WindowsRuntimeConfig>,
        gateway_pid: Option<u32>,
        message: &str,
    ) -> Self {
        Self {
            schema_version: 1,
            state,
            updated_at: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_default(),
            service_pid: std::process::id(),
            gateway_pid,
            onboarding_complete: config
                .map(|config| config.onboarding_complete)
                .unwrap_or(false),
            ports: config.map(|config| ServicePorts {
                admin: config.admin_port,
                backend: config.backend_port,
                auth: config.auth_port,
                grpc: config.grpc_port,
                proxy: config.proxy_port,
            }),
            listener_scope: config.map(|config| config.listener_scope.clone()),
            message: sanitize_status_message(message),
        }
    }
}

fn sanitize_status_message(message: &str) -> String {
    message
        .chars()
        .filter(|character| !character.is_control() || *character == ' ')
        .take(512)
        .collect()
}

fn write_status(paths: &WindowsPaths, state: &ServiceStateFile) -> anyhow::Result<()> {
    atomic_write_json(&paths.status, state)
}

fn write_emergency_status(message: &str) -> anyhow::Result<()> {
    let paths = WindowsPaths::discover()?;
    paths.create_runtime_directories()?;
    write_status(&paths, &ServiceStateFile::faulted(None, None, message))
}

fn ensure_faulted_status(message: &str) -> anyhow::Result<()> {
    let paths = WindowsPaths::discover()?;
    let current_process_fault = fs::read(&paths.status)
        .ok()
        .and_then(|content| serde_json::from_slice::<serde_json::Value>(&content).ok())
        .is_some_and(|status| {
            status.get("state").and_then(serde_json::Value::as_str) == Some("faulted")
                && status
                    .get("service_pid")
                    .and_then(serde_json::Value::as_u64)
                    == Some(u64::from(std::process::id()))
        });
    if current_process_fault {
        Ok(())
    } else {
        write_emergency_status(message)
    }
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context("serialize runtime JSON")?;
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("state"),
        std::process::id()
    ));
    let mut file = fs::File::create(&temp)
        .with_context(|| format!("create temporary file {}", temp.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let temp_wide = wide_null(&temp);
    let path_wide = wide_null(path);
    // SAFETY: both pointers reference valid, NUL-terminated UTF-16 buffers for
    // the duration of the call. Source and destination are on the same volume.
    let moved = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            path_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        let error = std::io::Error::last_os_error();
        let _ = fs::remove_file(&temp);
        return Err(error).with_context(|| format!("replace {}", path.display()));
    }
    Ok(())
}

fn wide_null(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

struct JobObject(HANDLE);

impl JobObject {
    fn new() -> anyhow::Result<Self> {
        // SAFETY: no security attributes or name are supplied; Windows returns
        // an owned handle which is closed by Drop.
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error()).context("create gateway Job Object");
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the information pointer and byte length exactly match the
        // JobObjectExtendedLimitInformation contract.
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            let error = std::io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            return Err(error).context("configure gateway Job Object");
        }
        Ok(Self(handle))
    }

    fn assign(&self, child: &tokio::process::Child) -> anyhow::Result<()> {
        let process_handle = child
            .raw_handle()
            .context("gateway process exited before Job Object assignment")?
            as HANDLE;
        // SAFETY: both handles are valid for this call and remain owned by
        // their respective RAII wrappers.
        if unsafe { AssignProcessToJobObject(self.0, process_handle) } == 0 {
            return Err(std::io::Error::last_os_error())
                .context("assign gateway process to Job Object");
        }
        Ok(())
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        // SAFETY: self.0 is an owned handle created by CreateJobObjectW.
        unsafe { CloseHandle(self.0) };
    }
}

fn enforce_service_account() -> anyhow::Result<()> {
    let whoami = trusted_system_directory()?.join("whoami.exe");
    let output = StdCommand::new(&whoami)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("query Windows service identity with {}", whoami.display()))?;
    let identity = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase();
    if identity != r"nt service\fnknock" {
        anyhow::bail!("FnKnock must run as NT SERVICE\\FnKnock; current identity is {identity:?}");
    }
    Ok(())
}

fn report_status(
    handle: &ServiceStatusHandle,
    current_state: ServiceState,
    controls_accepted: ServiceControlAccept,
    exit_code: ServiceExitCode,
    checkpoint: u32,
    wait_hint: Duration,
) -> anyhow::Result<()> {
    handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state,
            controls_accepted,
            exit_code,
            checkpoint,
            wait_hint,
            process_id: None,
        })
        .context("report FnKnock service status")
}

fn install_service() -> anyhow::Result<()> {
    let executable = env::current_exe().context("resolve fn-knock-service executable")?;
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;
    let info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: SERVICE_TYPE,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: executable.clone(),
        launch_arguments: vec!["--service".into()],
        dependencies: Vec::new(),
        account_name: Some(SERVICE_ACCOUNT.into()),
        account_password: None,
    };
    let access = ServiceAccess::CHANGE_CONFIG
        | ServiceAccess::QUERY_CONFIG
        | ServiceAccess::QUERY_STATUS
        | ServiceAccess::START
        | ServiceAccess::STOP
        | ServiceAccess::DELETE;
    let service = manager
        .create_service(&info, access)
        .or_else(|_| manager.open_service(SERVICE_NAME, access))?;
    service.change_config(&info)?;
    service.set_config_service_sid_info(ServiceSidType::Unrestricted)?;
    service.set_description(
        "fn-knock local gateway and authentication control plane (Windows x86_64)",
    )?;
    service.update_failure_actions(ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 60 * 60)),
        reboot_msg: Some(OsString::new()),
        command: Some(OsString::new()),
        actions: Some(
            SCM_RESTART_DELAYS_SECONDS
                .into_iter()
                .map(|seconds| ServiceAction {
                    action_type: ServiceActionType::Restart,
                    delay: Duration::from_secs(seconds),
                })
                .collect(),
        ),
    })?;
    // Configuration/port errors are reported as a clean SERVICE_STOPPED state
    // and must not enter a recovery loop. Only an unreported process crash
    // triggers the actions above.
    service.set_failure_actions_on_non_crash_failures(false)?;

    let paths = WindowsPaths::discover()?;
    paths.create_runtime_directories()?;
    configure_program_data_acl(&paths)?;
    let _ = load_runtime_config(&paths.runtime_config)?;
    configure_firewall_rule(&executable)?;
    println!("installed {SERVICE_NAME} as {SERVICE_ACCOUNT}");
    Ok(())
}

fn start_service() -> anyhow::Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(SERVICE_NAME, ServiceAccess::START)?;
    service.start::<&str>(&[])?;
    Ok(())
}

fn stop_service() -> anyhow::Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    )?;
    if service.query_status()?.current_state != ServiceState::Stopped {
        let _ = service.stop()?;
    }
    wait_for_service_stopped(&service, Duration::from_secs(20))
}

fn uninstall_service() -> anyhow::Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE,
    )?;
    if service.query_status()?.current_state != ServiceState::Stopped {
        let _ = service.stop();
    }
    wait_for_service_stopped(&service, Duration::from_secs(20))?;
    service.delete()?;
    let _ = run_system_checked(
        "netsh.exe",
        &[
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            &format!("name={FIREWALL_RULE_NAME}"),
        ],
    );
    println!("uninstalled {SERVICE_NAME}; ProgramData was retained");
    Ok(())
}

fn wait_for_service_stopped(service: &Service, timeout: Duration) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if service.query_status()?.current_state == ServiceState::Stopped {
            return Ok(());
        }
        if Instant::now() >= deadline {
            anyhow::bail!("FnKnock did not stop within {} seconds", timeout.as_secs());
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn configure_program_data_acl(paths: &WindowsPaths) -> anyhow::Result<()> {
    let root = paths.root.to_string_lossy().into_owned();
    let root_reset_args = [root.as_str(), "/reset", "/T", "/L"];
    run_system_checked("icacls.exe", &root_reset_args)?;
    let mut root_args = vec![root.as_str(), "/inheritance:r", "/grant:r"];
    root_args.extend_from_slice(ROOT_ACL_GRANTS);
    root_args.push("/L");
    run_system_checked("icacls.exe", &root_args)?;
    let state = paths.state.to_string_lossy().into_owned();
    let state_reset_args = [state.as_str(), "/reset", "/T", "/L"];
    run_system_checked("icacls.exe", &state_reset_args)?;
    let mut state_args = vec![state.as_str(), "/inheritance:r", "/grant:r"];
    state_args.extend_from_slice(STATE_ACL_GRANTS);
    state_args.push("/L");
    // Built-in Users are intentionally granted only this subtree. Windows'
    // default Bypass Traverse Checking privilege permits opening the known
    // status path without exposing listings or files under the protected root.
    run_system_checked("icacls.exe", &state_args)?;
    let rollback = paths.rollback.to_string_lossy().into_owned();
    let rollback_reset_args = [rollback.as_str(), "/reset", "/T", "/L"];
    run_system_checked("icacls.exe", &rollback_reset_args)?;
    let mut rollback_args = vec![rollback.as_str(), "/inheritance:r", "/grant:r"];
    rollback_args.extend_from_slice(ROLLBACK_ACL_GRANTS);
    rollback_args.push("/L");
    // The running service never reads installer snapshots or transaction
    // markers. Keep this subtree writable only by SYSTEM and administrators.
    run_system_checked("icacls.exe", &rollback_args)
}

fn configure_firewall_rule(service_executable: &Path) -> anyhow::Result<()> {
    let gateway = service_executable
        .parent()
        .context("service executable has no parent directory")?
        .join("fn-knock-gateway.exe");
    let _ = run_system_checked(
        "netsh.exe",
        &[
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            &format!("name={FIREWALL_RULE_NAME}"),
        ],
    );
    run_system_checked(
        "netsh.exe",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={FIREWALL_RULE_NAME}"),
            "dir=in",
            "action=allow",
            &format!("program={}", gateway.display()),
            "enable=yes",
            "profile=domain,private",
        ],
    )
}

fn run_system_checked(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let executable = trusted_system_directory()?.join(program);
    if !executable.is_file() {
        anyhow::bail!(
            "required Windows system tool is missing: {}",
            executable.display()
        );
    }
    let status = StdCommand::new(&executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("run {}", executable.display()))?;
    if !status.success() {
        anyhow::bail!("{program} failed with {status}");
    }
    Ok(())
}

fn display_exit_status(result: std::io::Result<std::process::ExitStatus>) -> String {
    result
        .map(|status| status.to_string())
        .unwrap_or_else(|error| format!("status unavailable: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_users_are_granted_only_status_tree_access() {
        assert!(
            !ROOT_ACL_GRANTS
                .iter()
                .any(|grant| grant.contains("S-1-5-32-545"))
        );
        assert!(
            STATE_ACL_GRANTS
                .iter()
                .any(|grant| *grant == "*S-1-5-32-545:(OI)(CI)RX")
        );
        assert!(
            !ROLLBACK_ACL_GRANTS
                .iter()
                .any(|grant| grant.contains("NT SERVICE\\FnKnock"))
        );
        for grants in [ROOT_ACL_GRANTS, STATE_ACL_GRANTS, ROLLBACK_ACL_GRANTS] {
            assert!(grants.contains(&"*S-1-5-18:F"));
            assert!(grants.contains(&"*S-1-5-18:(OI)(CI)F"));
            assert!(grants.contains(&"*S-1-5-32-544:F"));
            assert!(grants.contains(&"*S-1-5-32-544:(OI)(CI)F"));
        }
        assert!(ROOT_ACL_GRANTS.contains(&r"NT SERVICE\FnKnock:M"));
        assert!(STATE_ACL_GRANTS.contains(&"*S-1-5-32-545:RX"));
        assert_eq!(ROLLBACK_ACL_GRANTS.len(), 4);
    }

    #[test]
    fn scm_crash_recovery_delays_match_runtime_group_policy() {
        assert_eq!(SCM_RESTART_DELAYS_SECONDS, [5, 30, 60]);
    }

    #[test]
    fn legacy_runtime_config_defaults_onboarding_to_incomplete() {
        let config: WindowsRuntimeConfig = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "admin_port": 7991,
                "backend_port": 7998,
                "auth_port": 7997,
                "grpc_port": 7996,
                "proxy_port": 7999,
                "listener_scope": "loopback"
            }"#,
        )
        .unwrap();

        assert!(!config.onboarding_complete);
        assert!(!ServiceStateFile::starting(&config, None, "test").onboarding_complete);
    }

    #[test]
    fn runtime_config_defaults_gateway_to_all_interfaces() {
        assert_eq!(WindowsRuntimeConfig::default().listener_scope, "all");
    }

    #[test]
    fn legacy_loopback_gateway_config_is_migrated_to_all_interfaces() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("runtime.json");
        fs::write(
            &path,
            r#"{
                "schema_version": 1,
                "admin_port": 7991,
                "backend_port": 7998,
                "auth_port": 7997,
                "grpc_port": 7996,
                "proxy_port": 7999,
                "listener_scope": "loopback"
            }"#,
        )
        .unwrap();

        let config = load_runtime_config(&path).unwrap();
        assert_eq!(config.listener_scope, "all");

        let persisted: WindowsRuntimeConfig =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted.listener_scope, "all");
    }
}
