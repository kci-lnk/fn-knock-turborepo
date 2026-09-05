use super::*;

pub(super) const CONFIG_KEY: &str = "fn_knock:config";
pub(super) const HOST_MAPPINGS_GENERATION_KEY: &str = "fn_knock:config:host_mappings:generation";
pub(crate) const CONFIG_GENERATION_MARKER: &str = "__fn_knock_internal_host_mappings_generation";

pub(crate) fn strip_internal_config_metadata(config: &mut Value) {
    if let Some(object) = config.as_object_mut() {
        object.remove(CONFIG_GENERATION_MARKER);
    }
}

pub(crate) fn referenced_host_ipset_policy_ids<'a>(
    mappings: impl IntoIterator<Item = &'a Value>,
) -> BTreeSet<String> {
    let mut referenced = BTreeSet::new();
    for mapping in mappings {
        if let Some(id) = mapping
            .pointer("/visibility/policy_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            referenced.insert(id.to_string());
        }
        for condition in mapping
            .pointer("/advanced_auth/groups")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|group| group.get("conditions").and_then(Value::as_array))
            .flatten()
        {
            if let Some(id) = condition
                .get("policy_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                referenced.insert(id.to_string());
            }
        }
    }
    referenced
}

#[derive(Clone)]
pub struct Store {
    pub(super) manager: ConnectionManager,
    pub(super) path: PathBuf,
    pub(super) config_snapshot: Arc<ArcSwap<Value>>,
    pub(super) config_snapshot_revision: Arc<StdMutex<u64>>,
    pub(crate) auth_account_mutation_lock: Arc<tokio::sync::Mutex<()>>,
    pub(super) typed: TypedRepositories,
    pub(super) typed_config_primary_bootstrapped: Arc<AtomicBool>,
    pub(super) typed_config_shadow: ShadowTracker,
    pub(super) typed_docker_admin_shadow: ShadowTracker,
    pub(super) typed_event_dedupe_shadow: ShadowTracker,
    pub(super) typed_events_shadow: ShadowTracker,
    pub(super) typed_fnos_share_shadow: ShadowTracker,
    pub(super) typed_hmac_nonce_shadow: ShadowTracker,
    pub(super) typed_identity_runtime_shadow: ShadowTracker,
    pub(super) typed_login_backoff_shadow: ShadowTracker,
    pub(super) typed_mobility_shadow: ShadowTracker,
    pub(super) typed_notification_runtime_shadow: ShadowTracker,
    pub(super) typed_passkey_runtime_shadow: ShadowTracker,
    pub(super) typed_subdomain_grant_shadow: ShadowTracker,
    pub(super) typed_subdomain_rate_limit_shadow: ShadowTracker,
    pub(super) typed_whitelist_shadow: ShadowTracker,
    pub(super) typed_whitelist_runtime_shadow: ShadowTracker,
    pub(super) typed_wol_cooldown_shadow: ShadowTracker,
}

#[derive(Clone)]
pub(super) struct TypedRepositories {
    pub(super) typed_config: TypedConfigRepository,
    pub(super) typed_docker_admin: TypedDockerAdminRepository,
    pub(super) typed_event_dedupe: TypedEventDedupeRepository,
    pub(super) typed_events: TypedEventRepository,
    pub(super) typed_fnos_share: TypedFnosShareRepository,
    pub(super) typed_hmac_nonce: TypedHmacNonceRepository,
    pub(super) typed_identity_runtime: TypedIdentityRuntimeRepository,
    pub(super) typed_login_backoff: TypedLoginBackoffRepository,
    pub(super) typed_mobility: TypedMobilityRepository,
    pub(super) typed_notification_runtime: TypedNotificationRuntimeRepository,
    pub(super) typed_notifications: TypedNotificationRepository,
    pub(super) typed_passkey_runtime: TypedPasskeyRuntimeRepository,
    pub(super) typed_subdomain_grant: TypedSubdomainGrantRepository,
    pub(super) typed_subdomain_rate_limit: TypedSubdomainRateLimitRepository,
    pub(super) typed_whitelist: TypedWhitelistRepository,
    pub(super) typed_whitelist_runtime: TypedWhitelistRuntimeRepository,
    pub(super) typed_wol_cooldown: TypedWolCooldownRepository,
}

#[derive(Clone)]
pub(super) struct ShadowTracker {
    phase: &'static str,
    healthy: Arc<AtomicBool>,
    mismatches: Arc<AtomicU64>,
}

impl ShadowTracker {
    pub(super) fn new(phase: &'static str) -> Self {
        Self {
            phase,
            healthy: Arc::new(AtomicBool::new(true)),
            mismatches: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: self.phase,
            healthy: self.healthy.load(AtomicOrdering::Acquire),
            mismatch_count: self.mismatches.load(AtomicOrdering::Acquire),
        }
    }

    pub(super) fn mark_healthy(&self) -> bool {
        !self.healthy.swap(true, AtomicOrdering::AcqRel)
    }

    pub(super) fn set_healthy(&self) {
        self.healthy.store(true, AtomicOrdering::Release);
    }

    pub(super) fn mark_mismatch(&self) -> bool {
        self.mismatches.fetch_add(1, AtomicOrdering::Relaxed);
        self.healthy.swap(false, AtomicOrdering::AcqRel)
    }

    #[cfg(test)]
    pub(super) fn mismatch_count(&self) -> u64 {
        self.mismatches.load(AtomicOrdering::Acquire)
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TypedConfigShadowStatus {
    pub(crate) phase: &'static str,
    pub(crate) healthy: bool,
    pub(crate) mismatch_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct LoginBackoffAttemptState {
    pub(super) ip: String,
    pub(super) attempts: i64,
    #[serde(default, rename = "lastAttempt")]
    pub(super) last_attempt: i64,
    #[serde(default, rename = "blockedUntil")]
    pub(super) blocked_until: Option<i64>,
}
