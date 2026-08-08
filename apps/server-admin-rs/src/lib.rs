pub mod app;

#[cfg(windows)]
pub mod windows_service;

pub(crate) mod grpc_proto {
    #![allow(dead_code)]
    tonic::include_proto!("fnknock.v1");
}

pub(crate) mod admin;
pub(crate) mod auth;
pub(crate) mod certificates;
pub(crate) mod config;
pub(crate) mod ddns;
pub(crate) mod discovery;
pub(crate) mod events;
pub(crate) mod gateway;
pub(crate) mod infra;
pub(crate) mod notifications;
pub(crate) mod runtime_health;
pub(crate) mod security;
pub(crate) mod shared;
pub(crate) mod storage;
pub(crate) mod system;
pub(crate) mod tunnels;
pub(crate) mod waf;
pub(crate) mod wol;

pub(crate) use admin::{control as admin_control, panel as admin_panel};
pub(crate) use app::cleanup_legacy_auth_log_storage;
pub(crate) use auth::{
    backoff, common_locations as common_auth_locations, cookies, fnos_share_bypass,
    hmac as hmac_auth, ldap as ldap_auth, mobility as auth_mobility, oidc_admin, oidc_runtime,
    passkey as passkey_runtime,
};
pub(crate) use certificates::{acme, auto_https, fnos_certificate_sync, ssl};
pub(crate) use config::runtime as runtime_config;
pub(crate) use ddns as ddns_status;
pub(crate) use discovery::{cidr, ip_location, ip_location_config, scan_assets, scanner};
pub(crate) use events as system_events;
pub(crate) use gateway::{
    deep_monitor, logs as gateway_logs, proxy_config, settings as gateway_settings,
};
pub(crate) use infra::{
    app_version, go_backend, i18n, memory, openapi_docs, response, runtime_profile, settings,
    state, static_files,
};
pub(crate) use security::{
    general_blacklist, overview as security_overview, ssh as ssh_security, whitelist,
};
pub(crate) use shared::{
    auth_mobility_keys, auth_session_keys, cloudflared_utils, crypto_utils, frp_utils, fs_utils,
    http_body, http_utils, json_utils, net_utils, node_compat, proxy_utils, text_utils, time_utils,
    unix, version_utils,
};
pub(crate) use storage::store;
pub(crate) use system::{
    dashboard, maintenance, system_assets, system_info, system_monitor, terminal, terminal_paths,
    update,
};
pub(crate) use tunnels::{cloudflared, frpc};
pub(crate) use wol::wol_routes;

#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        env,
        ffi::{OsStr, OsString},
        sync::{Mutex, MutexGuard},
    };

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        previous: Vec<(String, Option<OsString>)>,
    }

    impl EnvGuard {
        pub(crate) fn new(keys: &[&str]) -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
            let previous = keys
                .iter()
                .map(|key| ((*key).to_string(), env::var_os(key)))
                .collect();
            Self {
                _lock: lock,
                previous,
            }
        }

        pub(crate) fn set(&self, key: &str, value: impl AsRef<OsStr>) {
            // SAFETY: tests serialize every process-environment mutation through
            // ENV_LOCK and hold the guard until values are restored.
            unsafe {
                env::set_var(key, value);
            }
        }

        pub(crate) fn remove(&self, key: &str) {
            // SAFETY: tests serialize every process-environment mutation through
            // ENV_LOCK and hold the guard until values are restored.
            unsafe {
                env::remove_var(key);
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.iter().rev() {
                // SAFETY: EnvGuard owns the process-wide test environment lock
                // while restoring the values captured at construction time.
                unsafe {
                    if let Some(value) = value {
                        env::set_var(key, value);
                    } else {
                        env::remove_var(key);
                    }
                }
            }
        }
    }
}
