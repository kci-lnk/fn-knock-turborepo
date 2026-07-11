mod platform;
mod runtime;
mod windows;

use serde::Serialize;
use tauri::{
    AppHandle,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
struct UpdateMetadata {
    version: String,
    notes: Option<String>,
}

#[tauri::command]
async fn get_status() -> runtime::DesktopStatus {
    tauri::async_runtime::spawn_blocking(runtime::collect_status)
        .await
        .unwrap_or_else(|_| runtime::collect_status())
}

#[tauri::command]
async fn open_admin(app: AppHandle) -> Result<(), String> {
    windows::open_admin(&app)
}

#[tauri::command]
async fn start_service() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(platform::start_service)
        .await
        .map_err(|error| format!("service start task failed: {error}"))?
}

#[tauri::command]
async fn restart_service(app: AppHandle) -> Result<(), String> {
    if !windows::destroy_admin(&app) {
        return Err("failed to close the admin window before restarting the service".to_string());
    }
    tauri::async_runtime::spawn_blocking(platform::restart_service)
        .await
        .map_err(|error| format!("service restart task failed: {error}"))?
}

#[tauri::command]
async fn save_runtime_config(app: AppHandle, config: runtime::RuntimeConfig) -> Result<(), String> {
    if !windows::destroy_admin(&app) {
        return Err(
            "failed to close the admin window before changing runtime configuration".to_string(),
        );
    }
    tauri::async_runtime::spawn_blocking(move || runtime::save_runtime_config(&config))
        .await
        .map_err(|error| format!("configuration task failed: {error}"))?
}

#[tauri::command]
async fn check_for_update(app: AppHandle) -> Result<Option<UpdateMetadata>, String> {
    let update = app
        .updater()
        .map_err(|error| format!("updater is not configured: {error}"))?
        .check()
        .await
        .map_err(|error| format!("update check failed: {error}"))?;
    Ok(update.map(|update| UpdateMetadata {
        version: update.version,
        notes: update.body,
    }))
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|error| format!("updater is not configured: {error}"))?
        .check()
        .await
        .map_err(|error| format!("update check failed: {error}"))?
        .ok_or_else(|| "no update is available".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| format!("update install failed: {error}"))?;
    app.restart();
}

#[tauri::command]
async fn export_diagnostics() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(runtime::export_diagnostics)
        .await
        .map_err(|error| format!("diagnostic export task failed: {error}"))??
        .into_os_string()
        .into_string()
        .map_err(|_| "diagnostic path is not valid Unicode".to_string())
}

fn create_tray(app: &tauri::App) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "打开 FnKnock", true, None::<&str>)?;
    let status_item = MenuItem::with_id(app, "status", "运行状态", true, None::<&str>)?;
    let restart_item = MenuItem::with_id(app, "restart", "重启服务", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出界面", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open_item,
            &status_item,
            &restart_item,
            &separator,
            &quit_item,
        ],
    )?;

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("FnKnock 网关")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    if windows::open_admin(&app).is_err() {
                        windows::show_status(&app);
                    }
                });
            }
            "status" => windows::show_status(app),
            "restart" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    if !windows::destroy_admin(&app) {
                        windows::show_status(&app);
                        return;
                    }
                    let _ = platform::restart_service();
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                windows::show_best_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_best_window(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_status,
            open_admin,
            start_service,
            restart_service,
            save_runtime_config,
            check_for_update,
            install_update,
            export_diagnostics,
        ])
        .setup(|app| {
            create_tray(app)?;
            windows::start_admin_identity_monitor(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running FnKnock desktop application");
}
