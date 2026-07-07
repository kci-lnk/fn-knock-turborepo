use std::sync::Arc;

use anyhow::Context;
use serde_json::Value;
use tokio::sync::{Mutex, Notify, RwLock};

use crate::{
    auto_https::AutoHttpsRedirectManager, go_backend::GoBackendClient, redis_store::RedisStore,
    settings::Settings,
};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub settings: Settings,
    pub redis: RedisStore,
    #[allow(dead_code)]
    pub go_backend: GoBackendClient,
    pub fallback_client: reqwest::Client,
    pub auto_https: AutoHttpsRedirectManager,
    pub acme_install_state: RwLock<Option<Value>>,
    pub ddns_schedule_reload: Notify,
    pub fnos_network_tuning_update_lock: Mutex<()>,
}

impl AppState {
    pub async fn new(settings: Settings) -> anyhow::Result<Self> {
        let redis = RedisStore::connect(&settings.redis_url)
            .await
            .context("connect redis")?;
        let go_backend = GoBackendClient::new(
            settings.go_backend_base_url.clone(),
            settings.request_timeout,
        )?;
        let fallback_client = reqwest::Client::builder()
            .timeout(settings.request_timeout)
            .build()
            .context("build fallback http client")?;

        Ok(Self {
            inner: Arc::new(AppStateInner {
                settings,
                redis,
                go_backend,
                fallback_client,
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
