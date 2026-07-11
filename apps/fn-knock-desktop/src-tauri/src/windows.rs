use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::runtime;

const STATUS_WINDOW: &str = "status";
const ADMIN_WINDOW: &str = "admin";

fn focus(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn show_status(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(STATUS_WINDOW) {
        focus(&window);
    }
}

pub fn destroy_admin(app: &AppHandle) -> bool {
    app.get_webview_window(ADMIN_WINDOW)
        .is_none_or(|window| window.destroy().is_ok())
}

pub fn show_best_window(app: &AppHandle) {
    if app.get_webview_window(ADMIN_WINDOW).is_some() {
        if open_admin(app).is_err() {
            show_status(app);
        }
    } else {
        show_status(app);
    }
}

fn admin_window_matches(window: &WebviewWindow, port: u16) -> bool {
    window.url().is_ok_and(|url| {
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port_or_known_default() == Some(port)
    })
}

pub fn start_admin_identity_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut consecutive_failures = 0_u8;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let Some(window) = app.get_webview_window(ADMIN_WINDOW) else {
                consecutive_failures = 0;
                continue;
            };
            let config = runtime::load_public_runtime_config();
            let identity_trusted = config.as_ref().is_ok_and(|config| {
                config.onboarding_complete
                    && admin_window_matches(&window, config.admin_port)
                    && crate::platform::verify_service_listener(
                        runtime::SERVICE_NAME,
                        config.admin_port,
                    )
                    .is_ok()
            });
            if !identity_trusted {
                // A service/PID/URL identity failure is not a transient health
                // failure: destroy immediately so the page cannot talk to a
                // different process that claimed the loopback port.
                if window.destroy().is_ok() {
                    show_status(&app);
                    consecutive_failures = 0;
                } else {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                }
                continue;
            }
            let ready = config.is_ok_and(|config| runtime::check_ready(config.admin_port).0);
            if ready {
                consecutive_failures = 0;
                continue;
            }
            consecutive_failures = consecutive_failures.saturating_add(1);
            if consecutive_failures >= 2 {
                // Destroy the WebView rather than hiding it. A hidden page can
                // keep issuing requests if another process later claims the
                // loopback port after the service has stopped.
                if window.destroy().is_ok() {
                    show_status(&app);
                    consecutive_failures = 0;
                }
            }
        }
    });
}

pub fn open_admin(app: &AppHandle) -> Result<(), String> {
    let config = runtime::load_public_runtime_config()?;
    if !config.onboarding_complete {
        return Err("请先在状态窗口完成 Windows 首次设置".to_string());
    }
    if let Some(window) = app.get_webview_window(ADMIN_WINDOW) {
        let (ready, detail) = runtime::check_ready(config.admin_port);
        if ready && admin_window_matches(&window, config.admin_port) {
            focus(&window);
            if let Some(status) = app.get_webview_window(STATUS_WINDOW) {
                let _ = status.hide();
            }
            return Ok(());
        }
        if window.destroy().is_err() {
            return Err("failed to close an untrusted admin window".to_string());
        }
        if !ready {
            return Err(detail.unwrap_or_else(|| "FnKnock is not ready".to_string()));
        }
    }

    let (ready, detail) = runtime::check_ready(config.admin_port);
    if !ready {
        return Err(detail.unwrap_or_else(|| "FnKnock is not ready".to_string()));
    }

    let origin = format!("http://127.0.0.1:{}", config.admin_port);
    let url = origin
        .parse()
        .map_err(|error| format!("invalid admin URL: {error}"))?;
    let allowed_port = config.admin_port;
    let window = WebviewWindowBuilder::new(app, ADMIN_WINDOW, WebviewUrl::External(url))
        .title("FnKnock")
        .inner_size(1280.0, 820.0)
        .min_inner_size(920.0, 620.0)
        .center()
        .on_navigation(move |url| {
            let same_admin_origin = url.scheme() == "http"
                && url.host_str() == Some("127.0.0.1")
                && url.port_or_known_default() == Some(allowed_port);
            if same_admin_origin {
                return crate::platform::verify_service_listener(
                    runtime::SERVICE_NAME,
                    allowed_port,
                )
                .is_ok();
            }
            if matches!(url.scheme(), "http" | "https") {
                let _ = open::that(url.as_str());
            }
            url.as_str() == "about:blank"
        })
        .on_new_window(|url, _features| {
            if matches!(url.scheme(), "http" | "https") {
                let _ = open::that(url.as_str());
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .build()
        .map_err(|error| format!("failed to create admin window: {error}"))?;
    focus(&window);
    if let Some(status) = app.get_webview_window(STATUS_WINDOW) {
        let _ = status.hide();
    }
    Ok(())
}
