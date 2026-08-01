use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Context;
use serde_json::Value;
use tokio::sync::{Mutex, Notify, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    auto_https::AutoHttpsRedirectManager,
    cidr::IpSetRegistry,
    go_backend::GoBackendClient,
    runtime_health::RuntimeHealth,
    settings::Settings,
    storage::legacy_redis_migration::{self, LegacyRedisMigrationOptions},
    store::Store,
    tunnels::supervisor::TunnelSupervisorRegistry,
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
    /// Atomically published, immutable CIDR sets used by request hot paths.
    /// Storage retains semantic selections and compact policies; request
    /// handling never reparses large CIDR arrays.
    pub ipsets: IpSetRegistry,
    /// Serializes rebuilding and publishing the complete whitelist policy to
    /// the proxy snapshot and direct-mode firewall.
    pub whitelist_runtime_sync_lock: Mutex<()>,
    #[allow(dead_code)]
    pub go_backend: GoBackendClient,
    pub runtime_health: RuntimeHealth,
    pub fallback_client: reqwest::Client,
    pub asset_download_client: reqwest::Client,
    pub auto_https: AutoHttpsRedirectManager,
    pub acme_install_state: RwLock<Option<Value>>,
    pub ddns_schedule_reload: Notify,
    pub fnos_network_tuning_update_lock: Mutex<()>,
    /// Serializes the Go loopback listener, dual-stack firewall rules and
    /// persisted FN Connect WAF preference as one fail-open transaction.
    pub fnos_connect_waf_update_lock: Mutex<()>,
    pub fnos_connect_waf_notify: Notify,
    pub fnos_connect_waf_status: RwLock<Value>,
    pub fnos_certificate_sync_lock: Mutex<()>,
    pub fnos_certificate_sync_notify: Notify,
    pub fnos_certificate_sync_status: RwLock<Value>,
    /// Serializes automatic-backup configuration, archive creation, restores,
    /// and destructive maintenance so none of them can overwrite one another.
    pub automatic_backup_lock: Mutex<()>,
    /// Wakes the automatic-backup scheduler after settings or stored data
    /// change, avoiding a polling delay after the feature is enabled.
    pub automatic_backup_notify: Notify,
    /// Serializes the host-mapping config -> Go runtime transaction, including
    /// rollback and background metadata merges. Without this guard, two admin
    /// requests can persist in one order and reach the runtime in another.
    pub host_mappings_update_lock: Mutex<()>,
    /// Tracks whether the latest complete HostRules snapshot was accepted by
    /// the matching Go gateway. Readiness must not hide a failed config sync.
    pub gateway_config_synced: AtomicBool,
    /// Serializes protocol-mapping config, its standalone feature switch,
    /// gateway listeners, firewall rules, and rollback as one transaction.
    pub protocol_mapping_update_lock: Mutex<()>,
    /// Serializes rule-file/state mutations with gateway reloads so rollback
    /// cannot overwrite a concurrent WAF rule update.
    pub waf_rules_update_lock: Mutex<()>,
    /// Owns all supervised tunnel process actors for this application state.
    pub tunnel_supervisors: TunnelSupervisorRegistry,
    /// Serializes read-modify-write updates to the legacy aggregate tunnel
    /// runtime record shared by frpc and cloudflared supervisors.
    pub tunnel_runtime_update_lock: Mutex<()>,
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
        let runtime_health = RuntimeHealth::new(&settings.data_dir, &settings.runtime_target)
            .context("initialize runtime health diagnostics")?;
        let store = match Store::connect(&settings.sqlite_path).await {
            Ok(store) => {
                runtime_health.operational_log(
                    "INFO",
                    "storage",
                    "opened",
                    "sqlite_opened",
                    serde_json::Map::new(),
                );
                store
            }
            Err(error) => {
                runtime_health.operational_log(
                    "ERROR",
                    "storage",
                    "open_failed",
                    "sqlite_open_failed",
                    serde_json::Map::from_iter([(
                        "result".to_string(),
                        serde_json::json!("failed"),
                    )]),
                );
                runtime_health.flush_operational_log().await;
                return Err(error).context("open sqlite storage");
            }
        };
        if legacy_redis_migration::migration_allowed_for_runtime_target(&settings.runtime_target) {
            let migration = match legacy_redis_migration::migrate_if_available(
                &store,
                &settings.legacy_redis_url,
                LegacyRedisMigrationOptions {
                    require_source: false,
                    force: false,
                    cleanup_source: true,
                },
            )
            .await
            {
                Ok(migration) => migration,
                Err(error) => {
                    runtime_health.operational_log(
                        "ERROR",
                        "storage",
                        "migration_failed",
                        "legacy_redis_migration_failed",
                        serde_json::Map::from_iter([(
                            "result".to_string(),
                            serde_json::json!("failed"),
                        )]),
                    );
                    runtime_health.flush_operational_log().await;
                    return Err(error).context("migrate legacy Redis data into SQLite");
                }
            };
            tracing::info!("{}", migration.summary());
            runtime_health.operational_log(
                "INFO",
                "storage",
                "migration_completed",
                "legacy_redis_migration_completed",
                serde_json::Map::from_iter([("result".to_string(), serde_json::json!("success"))]),
            );
            store
                .refresh_config_snapshot()
                .await
                .context("refresh config snapshot after legacy Redis migration")?;
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
                ipsets: IpSetRegistry::default(),
                whitelist_runtime_sync_lock: Mutex::new(()),
                go_backend,
                runtime_health,
                fallback_client,
                asset_download_client,
                auto_https: AutoHttpsRedirectManager::new(),
                acme_install_state: RwLock::new(None),
                ddns_schedule_reload: Notify::new(),
                fnos_network_tuning_update_lock: Mutex::new(()),
                fnos_connect_waf_update_lock: Mutex::new(()),
                fnos_connect_waf_notify: Notify::new(),
                fnos_connect_waf_status: RwLock::new(serde_json::json!({
                    "effective": false,
                    "protected": false,
                    "detected_http_port": null,
                    "listener_port": null,
                    "ipv4_redirect_active": false,
                    "ipv6_redirect_active": false,
                    "ipv4_relay_redirect_active": false,
                    "ipv6_relay_redirect_active": false,
                    "ipv4_direct_redirect_active": false,
                    "ipv6_direct_redirect_active": false,
                    "listener_guard_active": false,
                    "local_networks": null,
                    "waf_active": false,
                    "waf_mode": null,
                    "source": null,
                    "last_sync_at": null,
                    "last_error": null
                })),
                fnos_certificate_sync_lock: Mutex::new(()),
                fnos_certificate_sync_notify: Notify::new(),
                fnos_certificate_sync_status: RwLock::new(serde_json::json!({
                    "running": false,
                    "last_sync_at": null,
                    "last_result": null,
                    "last_error": null,
                    "failed_target_ids": []
                })),
                automatic_backup_lock: Mutex::new(()),
                automatic_backup_notify: Notify::new(),
                host_mappings_update_lock: Mutex::new(()),
                gateway_config_synced: AtomicBool::new(false),
                protocol_mapping_update_lock: Mutex::new(()),
                waf_rules_update_lock: Mutex::new(()),
                tunnel_supervisors: TunnelSupervisorRegistry::default(),
                tunnel_runtime_update_lock: Mutex::new(()),
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

impl AppState {
    pub(crate) fn set_gateway_config_synced(&self, synced: bool) {
        self.gateway_config_synced.store(synced, Ordering::Release);
    }

    pub(crate) fn gateway_config_synced(&self) -> bool {
        self.gateway_config_synced.load(Ordering::Acquire)
    }
}
