use std::sync::Arc;

use anyhow::Context;
use serde_json::Value;
use tokio::sync::{Mutex, Notify, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    auto_https::AutoHttpsRedirectManager,
    go_backend::GoBackendClient,
    settings::Settings,
    storage::legacy_redis_migration::{self, LegacyRedisMigrationOptions},
    store::Store,
};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub settings: Settings,
    /// Process-wide cooperative shutdown signal. Long-running background
    /// workers should observe this token instead of relying on runtime drop.
    pub shutdown: CancellationToken,
    pub store: Store,
    #[allow(dead_code)]
    pub go_backend: GoBackendClient,
    pub fallback_client: reqwest::Client,
    pub asset_download_client: reqwest::Client,
    pub auto_https: AutoHttpsRedirectManager,
    pub acme_install_state: RwLock<Option<Value>>,
    pub ddns_schedule_reload: Notify,
    pub fnos_network_tuning_update_lock: Mutex<()>,
    pub fnos_certificate_sync_lock: Mutex<()>,
    pub fnos_certificate_sync_notify: Notify,
    pub fnos_certificate_sync_status: RwLock<Value>,
    /// Serializes the host-mapping config -> Go runtime transaction, including
    /// rollback and background metadata merges. Without this guard, two admin
    /// requests can persist in one order and reach the runtime in another.
    pub host_mappings_update_lock: Mutex<()>,
    /// Serializes rule-file/state mutations with gateway reloads so rollback
    /// cannot overwrite a concurrent WAF rule update.
    pub waf_rules_update_lock: Mutex<()>,
}

impl AppState {
    #[allow(dead_code)]
    pub async fn new(settings: Settings) -> anyhow::Result<Self> {
        Self::new_with_shutdown(settings, CancellationToken::new()).await
    }

    pub async fn new_with_shutdown(
        settings: Settings,
        shutdown: CancellationToken,
    ) -> anyhow::Result<Self> {
        let store = Store::connect(&settings.sqlite_path)
            .await
            .context("open sqlite storage")?;
        if legacy_redis_migration::migration_allowed_for_runtime_target(&settings.runtime_target) {
            let migration = legacy_redis_migration::migrate_if_available(
                &store,
                &settings.legacy_redis_url,
                LegacyRedisMigrationOptions {
                    require_source: false,
                    force: false,
                    cleanup_source: true,
                },
            )
            .await
            .context("migrate legacy Redis data into SQLite")?;
            tracing::info!("{}", migration.summary());
        } else {
            tracing::info!("legacy Redis migration disabled for fpk-lite runtime");
        }
        let go_backend = GoBackendClient::new(
            settings.go_backend_grpc_addr.clone(),
            settings.internal_rpc_token.clone(),
            settings.request_timeout,
        )?;
        let fallback_client = reqwest::Client::builder()
            .timeout(settings.request_timeout)
            .build()
            .context("build fallback http client")?;
        // Large binary downloads use per-read timeouts so slow but active transfers can finish.
        let asset_download_client = reqwest::Client::builder()
            .connect_timeout(settings.asset_download_connect_timeout)
            .read_timeout(settings.asset_download_read_timeout)
            .no_gzip()
            .build()
            .context("build asset download http client")?;

        Ok(Self {
            inner: Arc::new(AppStateInner {
                settings,
                shutdown,
                store,
                go_backend,
                fallback_client,
                asset_download_client,
                auto_https: AutoHttpsRedirectManager::new(),
                acme_install_state: RwLock::new(None),
                ddns_schedule_reload: Notify::new(),
                fnos_network_tuning_update_lock: Mutex::new(()),
                fnos_certificate_sync_lock: Mutex::new(()),
                fnos_certificate_sync_notify: Notify::new(),
                fnos_certificate_sync_status: RwLock::new(serde_json::json!({
                    "running": false,
                    "last_sync_at": null,
                    "last_result": null,
                    "last_error": null,
                    "failed_target_ids": []
                })),
                host_mappings_update_lock: Mutex::new(()),
                waf_rules_update_lock: Mutex::new(()),
            }),
        })
    }
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
