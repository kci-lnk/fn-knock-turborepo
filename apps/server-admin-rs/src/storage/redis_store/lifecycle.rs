use super::*;

impl Store {
    pub(crate) fn diagnostics(&self) -> Arc<crate::runtime_health::operations::OperationRecorder> {
        self.manager.diagnostics()
    }

    pub async fn connect(sqlite_path: impl AsRef<Path>) -> crate::storage::StorageResult<Self> {
        let path = sqlite_path.as_ref().to_path_buf();
        let manager = ConnectionManager::open(&path).await?;
        let typed_config = TypedConfigRepository::new(manager.clone());
        typed_config.initialize().await?;
        let typed_docker_admin = TypedDockerAdminRepository::new(manager.clone());
        typed_docker_admin.initialize().await?;
        let typed_event_dedupe = TypedEventDedupeRepository::new(manager.clone());
        typed_event_dedupe.initialize().await?;
        let typed_events = TypedEventRepository::new(manager.clone());
        typed_events.initialize().await?;
        let typed_fnos_share = TypedFnosShareRepository::new(manager.clone());
        typed_fnos_share.initialize().await?;
        let typed_hmac_nonce = TypedHmacNonceRepository::new(manager.clone());
        typed_hmac_nonce.initialize().await?;
        let typed_identity_runtime = TypedIdentityRuntimeRepository::new(manager.clone());
        typed_identity_runtime.initialize().await?;
        let typed_login_backoff = TypedLoginBackoffRepository::new(manager.clone());
        typed_login_backoff.initialize().await?;
        let typed_mobility = TypedMobilityRepository::new(manager.clone());
        typed_mobility.initialize().await?;
        let typed_notification_runtime = TypedNotificationRuntimeRepository::new(manager.clone());
        typed_notification_runtime.initialize().await?;
        let typed_notifications = TypedNotificationRepository::new(manager.clone());
        typed_notifications.initialize().await?;
        let typed_passkey_runtime = TypedPasskeyRuntimeRepository::new(manager.clone());
        typed_passkey_runtime.initialize().await?;
        let typed_subdomain_grant = TypedSubdomainGrantRepository::new(manager.clone());
        typed_subdomain_grant.initialize().await?;
        let typed_subdomain_rate_limit = TypedSubdomainRateLimitRepository::new(manager.clone());
        typed_subdomain_rate_limit.initialize().await?;
        let typed_whitelist = TypedWhitelistRepository::new(manager.clone());
        typed_whitelist.initialize().await?;
        let typed_whitelist_runtime = TypedWhitelistRuntimeRepository::new(manager.clone());
        typed_whitelist_runtime.initialize().await?;
        let typed_wol_cooldown = TypedWolCooldownRepository::new(manager.clone());
        typed_wol_cooldown.initialize().await?;
        let store = Self {
            manager,
            path,
            config_snapshot: Arc::new(ArcSwap::from_pointee(default_config())),
            config_snapshot_revision: Arc::new(StdMutex::new(0)),
            auth_account_mutation_lock: Arc::new(tokio::sync::Mutex::new(())),
            typed: TypedRepositories {
                typed_config,
                typed_docker_admin,
                typed_event_dedupe,
                typed_events,
                typed_fnos_share,
                typed_hmac_nonce,
                typed_identity_runtime,
                typed_login_backoff,
                typed_mobility,
                typed_notification_runtime,
                typed_notifications,
                typed_passkey_runtime,
                typed_subdomain_grant,
                typed_subdomain_rate_limit,
                typed_whitelist,
                typed_whitelist_runtime,
                typed_wol_cooldown,
            },
            typed_config_primary_bootstrapped: Arc::new(AtomicBool::new(false)),
            typed_config_shadow: ShadowTracker::new("typed_primary"),
            typed_docker_admin_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_event_dedupe_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_events_shadow: ShadowTracker::new("typed_primary"),
            typed_fnos_share_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_hmac_nonce_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_identity_runtime_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_login_backoff_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_mobility_shadow: ShadowTracker::new("dual_write_shadow"),
            typed_notification_runtime_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_passkey_runtime_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_subdomain_grant_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_subdomain_rate_limit_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_whitelist_shadow: ShadowTracker::new("typed_primary"),
            typed_whitelist_runtime_shadow: ShadowTracker::new("legacy_primary_shadow"),
            typed_wol_cooldown_shadow: ShadowTracker::new("legacy_primary_shadow"),
        };
        store.typed.typed_docker_admin.rebuild_from_legacy().await?;
        store.typed.typed_event_dedupe.rebuild_from_legacy().await?;
        store.rebuild_typed_system_events_from_legacy().await?;
        store.typed.typed_fnos_share.rebuild_from_legacy().await?;
        store.typed.typed_hmac_nonce.rebuild_from_legacy().await?;
        store
            .typed
            .typed_identity_runtime
            .rebuild_from_legacy()
            .await?;
        store
            .typed
            .typed_login_backoff
            .rebuild_from_legacy()
            .await?;
        store.typed.typed_mobility.rebuild_from_legacy().await?;
        store
            .typed
            .typed_notification_runtime
            .rebuild_from_legacy()
            .await?;
        store
            .rebuild_typed_notification_documents_from_legacy()
            .await?;
        store
            .rebuild_typed_notification_history_from_legacy()
            .await?;
        store
            .typed
            .typed_passkey_runtime
            .rebuild_from_legacy()
            .await?;
        store
            .typed
            .typed_subdomain_grant
            .rebuild_from_legacy()
            .await?;
        store
            .typed
            .typed_subdomain_rate_limit
            .rebuild_from_legacy()
            .await?;
        store.rebuild_typed_whitelist_from_legacy().await?;
        store
            .typed
            .typed_whitelist_runtime
            .rebuild_from_legacy()
            .await?;
        store.typed.typed_wol_cooldown.rebuild_from_legacy().await?;
        store.refresh_config_snapshot().await?;
        Ok(store)
    }

    pub(crate) async fn prepare_for_system_update(
        &self,
        backup_path: impl AsRef<Path>,
    ) -> crate::storage::StorageResult<()> {
        self.manager
            .prepare_for_system_update(backup_path.as_ref())
            .await
    }

    pub(crate) async fn checkpoint_for_shutdown(&self) -> crate::storage::StorageResult<()> {
        self.manager.checkpoint_for_shutdown().await
    }

    pub(crate) async fn cancel_system_update(&self) -> crate::storage::StorageResult<()> {
        self.manager.cancel_system_update().await
    }

    pub(super) fn conn(&self) -> ConnectionManager {
        self.manager.clone()
    }

    pub(crate) fn primary_queue_status(&self) -> redis::PrimaryQueueStatus {
        self.manager.primary_queue_status()
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config_snapshot(&self) -> Arc<Value> {
        self.config_snapshot.load_full()
    }

    pub async fn refresh_config_snapshot(&self) -> crate::storage::StorageResult<()> {
        let (config, revision) = self.reconcile_typed_config_from_legacy().await?;
        self.publish_config_snapshot(config, revision);
        Ok(())
    }

    pub(super) fn publish_config_snapshot(&self, config: Value, revision: u64) {
        let mut published_revision = self
            .config_snapshot_revision
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if revision <= *published_revision {
            tracing::debug!(
                revision,
                current_revision = *published_revision,
                "ignored stale config snapshot publication"
            );
            return;
        }
        self.config_snapshot.store(Arc::new(config));
        *published_revision = revision;
        self.typed_config_shadow.set_healthy();
    }

    pub(crate) fn typed_config_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_config_shadow.status()
    }

    pub(crate) fn typed_mobility_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_mobility_shadow.status()
    }

    pub(crate) fn typed_login_backoff_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_login_backoff_shadow.status()
    }

    pub(crate) fn typed_docker_admin_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_docker_admin_shadow.status()
    }

    pub(crate) fn typed_event_dedupe_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_event_dedupe_shadow.status()
    }

    pub(crate) fn typed_identity_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_identity_runtime_shadow.status()
    }

    pub(crate) fn typed_fnos_share_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_fnos_share_shadow.status()
    }

    pub(crate) fn typed_hmac_nonce_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_hmac_nonce_shadow.status()
    }

    pub(crate) fn typed_subdomain_rate_limit_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_subdomain_rate_limit_shadow.status()
    }

    pub(crate) fn typed_subdomain_grant_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_subdomain_grant_shadow.status()
    }

    pub(crate) fn typed_wol_cooldown_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_wol_cooldown_shadow.status()
    }

    pub(crate) fn typed_whitelist_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_whitelist_runtime_shadow.status()
    }

    pub(crate) fn typed_notification_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_notification_runtime_shadow.status()
    }

    pub(crate) fn typed_passkey_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        self.typed_passkey_runtime_shadow.status()
    }

    #[cfg(test)]
    pub(crate) fn typed_config_shadow_mismatch_count(&self) -> u64 {
        self.typed_config_shadow_status().mismatch_count
    }
}
