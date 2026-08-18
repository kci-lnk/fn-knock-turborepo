use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use std::{collections::HashMap, future::Future, time::Duration};

use anyhow::Context;
use serde_json::Value;
use tokio::{
    sync::{Mutex, Notify, RwLock, Semaphore, broadcast, watch},
    task::AbortHandle,
};
use tokio_util::sync::CancellationToken;

use super::background_tasks::BackgroundTaskRegistry;
use crate::{
    auto_https::AutoHttpsRedirectManager,
    cidr::IpSetRegistry,
    go_backend::GoBackendClient,
    runtime_health::RuntimeHealth,
    settings::Settings,
    static_files::StaticFileCatalogs,
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
    /// Tracks named application-owned tasks so shutdown can wait, report, and
    /// abort workers that fail to observe the cancellation token.
    pub(crate) background_tasks: BackgroundTaskRegistry,
    /// Persistent data access and the compatibility keyspace used during the
    /// typed SQLite repository migration.
    pub storage: StorageState,
    /// Go control-plane client and config/runtime transaction ownership.
    pub gateway: GatewayState,
    /// Security policy snapshots and mutation locks.
    pub security: SecurityState,
    pub runtime_health: RuntimeHealth,
    pub static_files: StaticFileCatalogs,
    /// Cached server-authoritative locale used by index responses. Keeping it
    /// in memory avoids a typed/legacy storage reconciliation on every SPA
    /// navigation while locale writes and restore syncs update it explicitly.
    pub(crate) browser_locale: RwLock<String>,
    pub fallback_client: reqwest::Client,
    pub asset_download_client: reqwest::Client,
    pub auto_https: AutoHttpsRedirectManager,
    pub acme_install_state: RwLock<Option<Value>>,
    /// Owns cancellation and completion signals for ACME jobs started by this
    /// process. Persistent leases remain the cross-process source of truth;
    /// this registry is intentionally empty after a restart so stale leases
    /// can be identified and reconciled safely.
    pub(crate) acme_runtime: AcmeRuntimeState,
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
    /// Maintenance-owned synchronization and scheduler signals. Keeping these
    /// together makes lock ordering and task ownership explicit at the domain
    /// boundary instead of growing the process-wide state bag.
    pub maintenance: MaintenanceState,
    /// Wake-on-LAN locks, reload signals and non-secret runtime status.
    pub wol: WolState,
    /// Tunnel process ownership, runtime locks and Cloudflare scheduling state.
    pub tunnel: TunnelState,
}

pub struct GatewayState {
    #[allow(dead_code)]
    pub client: GoBackendClient,
    /// Serializes SSL library mutations. Certificate automation may run
    /// concurrently with manual and ACME updates, so their read-modify-write
    /// sequences must share one owner.
    pub ssl_update_lock: Mutex<()>,
    /// Serializes SSL gateway calls and lets every caller converge a stale
    /// deployment request to the newest persisted SSL configuration.
    pub ssl_deployment_lock: Mutex<()>,
    /// Serializes the host-mapping config -> Go runtime transaction, including
    /// rollback and background metadata merges. Without this guard, two admin
    /// requests can persist in one order and reach the runtime in another.
    pub host_mappings_update_lock: Mutex<()>,
    /// Serializes protocol-mapping config, its standalone feature switch,
    /// gateway listeners, firewall rules, and rollback as one transaction.
    pub protocol_mapping_update_lock: Mutex<()>,
    /// Serializes the persisted Go GC policy with runtime application and
    /// rollback. The guard is also shared by boot and backup-import syncs so a
    /// late background sync cannot overwrite a newer admin update.
    pub memory_update_lock: Mutex<()>,
    /// Tracks whether the latest complete HostRules snapshot was accepted by
    /// the matching Go gateway. Readiness must not hide a failed config sync.
    config_synced: AtomicBool,
}

pub struct StorageState {
    /// Existing Redis-compatible facade. Typed repositories will be added
    /// beside this facade and can shadow-compare without leaking migration
    /// details into unrelated runtime domains.
    pub store: Store,
}

#[derive(Clone)]
pub(crate) struct AcmeJobControl {
    pub(crate) cancellation: CancellationToken,
    pub(crate) finished: CancellationToken,
    pid: Arc<AtomicU32>,
}

impl AcmeJobControl {
    fn new(shutdown: &CancellationToken) -> Self {
        Self {
            cancellation: shutdown.child_token(),
            finished: CancellationToken::new(),
            pid: Arc::new(AtomicU32::new(0)),
        }
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid.load(Ordering::Acquire)
    }

    pub(crate) fn set_pid(&self, pid: u32) {
        self.pid.store(pid, Ordering::Release);
    }
}

#[derive(Default)]
pub(crate) struct AcmeRuntimeState {
    jobs: Mutex<HashMap<String, AcmeJobControl>>,
}

impl StorageState {
    fn new(store: Store) -> Self {
        Self { store }
    }
}

impl GatewayState {
    fn new(client: GoBackendClient) -> Self {
        Self {
            client,
            ssl_update_lock: Mutex::new(()),
            ssl_deployment_lock: Mutex::new(()),
            host_mappings_update_lock: Mutex::new(()),
            protocol_mapping_update_lock: Mutex::new(()),
            memory_update_lock: Mutex::new(()),
            config_synced: AtomicBool::new(false),
        }
    }
}

pub struct MaintenanceState {
    /// Serializes automatic-backup configuration, archive creation, restores,
    /// and destructive maintenance so none of them can overwrite one another.
    pub automatic_backup_lock: Mutex<()>,
    /// Bounds memory-heavy backup archive encoding and decoding to one job per
    /// process. Import mutation locks are intentionally acquired only after
    /// this short-lived archive work lock has been released.
    pub backup_archive_work_lock: Mutex<()>,
    /// Wakes the automatic-backup scheduler after settings or stored data
    /// change, avoiding a polling delay after the feature is enabled.
    pub automatic_backup_notify: Notify,
}

pub struct SecurityState {
    /// Atomically published, immutable CIDR sets used by request hot paths.
    /// Storage retains semantic selections and compact policies; request
    /// handling never reparses large CIDR arrays.
    pub ipsets: IpSetRegistry,
    /// Serializes rebuilding and publishing the complete whitelist policy to
    /// the proxy snapshot and direct-mode firewall.
    pub whitelist_runtime_sync_lock: Mutex<()>,
    /// Serializes CAPTCHA validation with its read-modify-write persistence so
    /// concurrent provider or nested difficulty patches cannot be lost.
    pub captcha_settings_update_lock: Mutex<()>,
    /// Serializes scanner policy writes so the main settings form and the
    /// independently managed path whitelist cannot overwrite one another.
    pub scanner_settings_update_lock: Mutex<()>,
    /// Serializes rule-file/state mutations with gateway reloads so rollback
    /// cannot overwrite a concurrent WAF rule update.
    pub waf_rules_update_lock: Mutex<()>,
    /// Serializes the lease -> persist -> acknowledge handoff for WAF events.
    /// Both the background task and the explicit UI drain endpoint use it.
    pub waf_event_drain_lock: Mutex<()>,
}

impl Default for SecurityState {
    fn default() -> Self {
        Self {
            ipsets: IpSetRegistry::default(),
            whitelist_runtime_sync_lock: Mutex::new(()),
            captcha_settings_update_lock: Mutex::new(()),
            scanner_settings_update_lock: Mutex::new(()),
            waf_rules_update_lock: Mutex::new(()),
            waf_event_drain_lock: Mutex::new(()),
        }
    }
}

impl Default for MaintenanceState {
    fn default() -> Self {
        Self {
            automatic_backup_lock: Mutex::new(()),
            backup_archive_work_lock: Mutex::new(()),
            automatic_backup_notify: Notify::new(),
        }
    }
}

pub struct WolState {
    /// Serializes Relay/Target metadata with installation-bound PSK files.
    pub config_lock: Mutex<()>,
    /// Serializes the persisted feature switch with Go portal runtime sync.
    pub feature_update_lock: Mutex<()>,
    /// Reloads the built-in Relay listener after its local configuration changes.
    pub relay_reload: Notify,
    /// Versioned wakeup for all WoL supervisors after the feature switch changes.
    /// A watch channel prevents changes from being lost between a config read and wait.
    pub runtime_reload: watch::Sender<u64>,
    /// Non-secret status for the built-in Relay listener.
    pub relay_status: RwLock<Value>,
    /// Process-local third-party connection state. Credentials and broker
    /// tokens never enter this map.
    pub integration_status: RwLock<HashMap<String, Value>>,
    /// Online-state changes consumed by third-party integrations.
    pub status_updates: broadcast::Sender<Value>,
    /// Bounds outbound SSH work so a burst of remote shutdown requests cannot
    /// exhaust sockets or crypto workers.
    pub ssh_concurrency: Semaphore,
}

impl Default for WolState {
    fn default() -> Self {
        Self {
            config_lock: Mutex::new(()),
            feature_update_lock: Mutex::new(()),
            relay_reload: Notify::new(),
            runtime_reload: watch::channel(0).0,
            relay_status: RwLock::new(serde_json::json!({
                "enabled": false,
                "active": false,
                "listenAddress": null,
                "lastError": null,
                "updatedAt": null
            })),
            integration_status: RwLock::new(HashMap::new()),
            status_updates: broadcast::channel(128).0,
            ssh_concurrency: Semaphore::new(8),
        }
    }
}

pub struct TunnelState {
    /// Owns all supervised tunnel process actors for this application state.
    pub supervisors: TunnelSupervisorRegistry,
    /// Serializes read-modify-write updates to the legacy aggregate tunnel
    /// runtime record shared by frpc and cloudflared supervisors.
    pub runtime_update_lock: Mutex<()>,
    /// Serializes Cloudflare discovery, preview/apply, DNS reconciliation and
    /// optimization cutovers so a scheduled run cannot race an admin action.
    pub cloudflared_manage_lock: Mutex<()>,
    /// In-memory preview cache. Plans intentionally do not survive a restart;
    /// every apply must be based on recently observed Cloudflare state.
    pub cloudflared_plans: Mutex<HashMap<String, Value>>,
    /// Bounded, non-secret reconciliation job state exposed to the admin UI.
    /// Jobs make long Cloudflare mutations independent of the request connection
    /// and provide idempotent recovery when a client misses the initial response.
    pub cloudflared_reconcile_jobs: RwLock<HashMap<String, Value>>,
    /// Bounded, non-secret optimization scan state exposed to the admin UI.
    pub cloudflared_scan_jobs: RwLock<HashMap<String, Value>>,
    /// Serializes manual and scheduled optimization scans so their combined
    /// probe concurrency and download traffic stay within the advertised cap.
    pub cloudflared_scan_lock: Mutex<()>,
    /// Wakes the Cloudflare reconciler after config, mapping or credential changes.
    pub cloudflared_schedule_notify: Notify,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self {
            supervisors: TunnelSupervisorRegistry::default(),
            runtime_update_lock: Mutex::new(()),
            cloudflared_manage_lock: Mutex::new(()),
            cloudflared_plans: Mutex::new(HashMap::new()),
            cloudflared_reconcile_jobs: RwLock::new(HashMap::new()),
            cloudflared_scan_jobs: RwLock::new(HashMap::new()),
            cloudflared_scan_lock: Mutex::new(()),
            cloudflared_schedule_notify: Notify::new(),
        }
    }
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
            .https_only(true)
            .redirect(reqwest::redirect::Policy::limited(10))
            .no_gzip()
            .build()
            .context("build asset download http client")?;
        let static_files =
            StaticFileCatalogs::build(&settings.admin_static_path, &settings.auth_static_path);
        let browser_locale = store
            .locale()
            .await
            .ok()
            .and_then(|locale| {
                locale
                    .get("default_locale")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .filter(|locale| {
                matches!(
                    locale.as_str(),
                    "zh-CN" | "zh-Hant" | "en" | "ko-KR" | "ja-JP"
                )
            })
            .unwrap_or_else(|| "zh-CN".to_string());

        Ok(Self {
            inner: Arc::new(AppStateInner {
                settings,
                shutdown,
                background_tasks: BackgroundTaskRegistry::default(),
                storage: StorageState::new(store),
                gateway: GatewayState::new(go_backend),
                security: SecurityState::default(),
                runtime_health,
                static_files,
                browser_locale: RwLock::new(browser_locale),
                fallback_client,
                asset_download_client,
                auto_https: AutoHttpsRedirectManager::new(),
                acme_install_state: RwLock::new(None),
                acme_runtime: AcmeRuntimeState::default(),
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
                maintenance: MaintenanceState::default(),
                wol: WolState::default(),
                tunnel: TunnelState::default(),
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
    pub(crate) async fn register_acme_job_control(&self, job_id: &str) -> Option<AcmeJobControl> {
        let mut jobs = self.acme_runtime.jobs.lock().await;
        if jobs.contains_key(job_id) {
            return None;
        }
        let control = AcmeJobControl::new(&self.shutdown);
        jobs.insert(job_id.to_string(), control.clone());
        Some(control)
    }

    pub(crate) async fn acme_job_control(&self, job_id: &str) -> Option<AcmeJobControl> {
        self.acme_runtime.jobs.lock().await.get(job_id).cloned()
    }

    pub(crate) async fn finish_acme_job_control(&self, job_id: &str) {
        if let Some(control) = self.acme_runtime.jobs.lock().await.remove(job_id) {
            control.set_pid(0);
            control.finished.cancel();
        }
    }

    pub(crate) fn spawn_background<F>(&self, name: &'static str, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.background_tasks.spawn(name, future);
    }

    pub(crate) fn spawn_abortable_background<F>(
        &self,
        name: &'static str,
        future: F,
    ) -> Option<AbortHandle>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.background_tasks.spawn_abortable(name, future)
    }

    pub(crate) async fn shutdown_background_tasks(&self, deadline: Duration) -> Vec<&'static str> {
        self.background_tasks.shutdown(deadline).await
    }

    pub(crate) fn set_gateway_config_synced(&self, synced: bool) {
        self.gateway.config_synced.store(synced, Ordering::Release);
    }

    pub(crate) fn gateway_config_synced(&self) -> bool {
        self.gateway.config_synced.load(Ordering::Acquire)
    }

    pub(crate) async fn set_browser_locale(&self, value: &Value) {
        let locale = value
            .get("default_locale")
            .and_then(Value::as_str)
            .filter(|locale| matches!(*locale, "zh-CN" | "zh-Hant" | "en" | "ko-KR" | "ja-JP"))
            .unwrap_or("zh-CN");
        *self.browser_locale.write().await = locale.to_string();
    }
}
