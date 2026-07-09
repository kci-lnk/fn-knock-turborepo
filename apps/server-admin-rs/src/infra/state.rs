use std::sync::Arc;

use anyhow::Context;
use serde_json::Value;
use tokio::sync::{Mutex, Notify, RwLock};

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
    pub store: Store,
    #[allow(dead_code)]
    pub go_backend: GoBackendClient,
    pub fallback_client: reqwest::Client,
    pub asset_download_client: reqwest::Client,
    pub auto_https: AutoHttpsRedirectManager,
    pub acme_install_state: RwLock<Option<Value>>,
    pub ddns_schedule_reload: Notify,
    pub fnos_network_tuning_update_lock: Mutex<()>,
}

impl AppState {
    pub async fn new(settings: Settings) -> anyhow::Result<Self> {
        let store = Store::connect(&settings.sqlite_path)
            .await
            .context("open sqlite storage")?;
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
                store,
                go_backend,
                fallback_client,
                asset_download_client,
                auto_https: AutoHttpsRedirectManager::new(),
                acme_install_state: RwLock::new(None),
                ddns_schedule_reload: Notify::new(),
                fnos_network_tuning_update_lock: Mutex::new(()),
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
