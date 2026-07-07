use std::env;

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod boot;
mod cli;
mod docker_admin_view;
mod router;
mod server;

pub(crate) use boot::cleanup_legacy_auth_log_storage;

use crate::{
    acme::start_acme_tasks,
    auth_mobility::start_auth_mobility_tasks,
    auto_https::sync_auto_https_on_boot,
    cloudflared::start_cloudflared_tasks,
    common_auth_locations::start_common_auth_location_tasks,
    dashboard::start_traffic_tasks,
    ddns_status::start_ddns_tasks,
    frpc::start_frpc_tasks,
    i18n::{DEFAULT_LOCALE, Translator},
    ip_location::start_ip_location_worker,
    notifications::start_notification_tasks,
    runtime_profile,
    settings::Settings,
    ssh_security::start_ssh_security_tasks,
    state::AppState,
    system_assets::start_system_clock_tasks,
    system_monitor::start_system_monitor_tasks,
    terminal::start_terminal_tasks,
    update::start_update_tasks,
    waf::start_waf_tasks,
    whitelist::start_whitelist_tasks,
};

pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("server_admin_rs=info,tower_http=info,axum=info")
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Some(command) = env::args().nth(1) {
        match command.as_str() {
            "reset-panel-password" | "reset-admin-panel-password" => {
                if let Err(error) = cli::reset_panel_password_command().await {
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
            "-h" | "--help" => {
                cli::print_help();
                return Ok(());
            }
            _ => anyhow::bail!("unknown command: {command}"),
        }
    }

    let settings = Settings::from_env();
    let state = AppState::new(settings.clone()).await?;
    sync_auto_https_on_boot(state.clone()).await;
    boot::start_boot_sync_tasks(state.clone());
    start_traffic_tasks(state.clone());
    start_ip_location_worker(state.clone());
    start_notification_tasks(state.clone());
    start_update_tasks(state.clone());
    start_ddns_tasks(state.clone());
    start_acme_tasks(state.clone());
    start_system_monitor_tasks(state.clone());
    start_system_clock_tasks(state.clone());
    start_auth_mobility_tasks(state.clone());
    start_common_auth_location_tasks(state.clone());
    start_whitelist_tasks(state.clone());
    start_waf_tasks(state.clone());
    start_terminal_tasks(state.clone());
    start_ssh_security_tasks(state.clone());
    start_cloudflared_tasks(state.clone());
    start_frpc_tasks(state.clone());

    let backend_addr = settings.backend_addr()?;
    let auth_addr = settings.auth_addr()?;
    let protected_admin_runtime = runtime_profile::admin_panel_protected_runtime(&state);
    let admin_view_addr = if protected_admin_runtime {
        settings.admin_view_addr()?
    } else {
        None
    };

    let backend = server::serve(
        "backend",
        backend_addr,
        router::backend_router(state.clone(), false),
    );
    let auth = server::serve("auth", auth_addr, router::auth_router(state.clone()));

    if let Some(addr) = admin_view_addr {
        let admin_view = server::serve("admin-view", addr, router::backend_router(state, true));
        tokio::try_join!(backend, auth, admin_view)?;
    } else {
        tokio::try_join!(backend, auth)?;
    }

    Ok(())
}
