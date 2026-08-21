use super::*;

#[derive(Clone, Debug)]
pub(super) struct CommandSpec {
    pub(super) name: String,
    pub(super) args: Vec<String>,
    pub(super) ignore: bool,
}

impl CommandSpec {
    pub(super) fn new(name: &str) -> Self {
        Self {
            name: name.to_ascii_uppercase(),
            args: Vec::new(),
            ignore: false,
        }
    }
}

pub(super) fn key_affects_typed_mobility(key: &str) -> bool {
    key.starts_with("fn_knock:session:")
        || key.starts_with("fn_knock:auth_mobility:")
        || key.starts_with(crate::storage::typed_login_backoff::LOGIN_BACKOFF_PREFIX)
        || key.starts_with(crate::storage::typed_docker_admin::SESSION_PREFIX)
        || key.starts_with(crate::storage::typed_docker_admin::LOGIN_BACKOFF_PREFIX)
        || key.starts_with(crate::storage::typed_event_dedupe::DEDUPE_PREFIX)
        || crate::storage::typed_fnos_share::owns_key(key)
        || key.starts_with(crate::storage::typed_hmac_nonce::NONCE_PREFIX)
        || key.starts_with(crate::storage::typed_subdomain_rate_limit::RATE_LIMIT_PREFIX)
        || key.starts_with(crate::storage::typed_wol_cooldown::COOLDOWN_PREFIX)
        || key.starts_with(crate::storage::typed_notification_runtime::LEASE_PREFIX)
        || key.starts_with(crate::storage::typed_notification_runtime::COOLDOWN_PREFIX)
        || key.starts_with(crate::storage::typed_notification_runtime::WINDOW_PREFIX)
        || key == crate::storage::typed_notification_runtime::READY_KEY
        || key.starts_with(crate::storage::typed_passkey_runtime::CHALLENGE_PREFIX)
        || key.starts_with(crate::storage::typed_passkey_runtime::STATE_PREFIX)
        || key.starts_with(crate::storage::typed_passkey_runtime::BIND_PREFIX)
        || crate::storage::typed_subdomain_grant::owns_key(key)
        || key.starts_with(crate::storage::typed_identity_runtime::OIDC_PREFIX)
        || key.starts_with(crate::storage::typed_identity_runtime::LDAP_PREFIX)
        || crate::storage::typed_whitelist_runtime::owns_key(key)
}

#[derive(Clone, Debug, Default)]
pub(super) enum TypedMobilitySyncScope {
    #[default]
    None,
    Keys(BTreeSet<String>),
    All,
}

impl TypedMobilitySyncScope {
    pub(super) fn from_key(key: &str) -> Self {
        if key_affects_typed_mobility(key) {
            Self::Keys(BTreeSet::from([key.to_string()]))
        } else {
            Self::None
        }
    }

    pub(super) fn from_keys(keys: impl IntoIterator<Item = String>) -> Self {
        let keys = keys
            .into_iter()
            .filter(|key| key_affects_typed_mobility(key))
            .collect::<BTreeSet<_>>();
        if keys.is_empty() {
            Self::None
        } else {
            Self::Keys(keys)
        }
    }

    pub(super) fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::All, _) | (_, Self::All) => Self::All,
            (Self::None, scope) | (scope, Self::None) => scope,
            (Self::Keys(mut left), Self::Keys(right)) => {
                left.extend(right);
                Self::Keys(left)
            }
        }
    }
}

pub(super) fn command_typed_mobility_scope(command: &CommandSpec) -> TypedMobilitySyncScope {
    let mutates = matches!(
        command.name.as_str(),
        "SET"
            | "SETEX"
            | "DEL"
            | "EXPIRE"
            | "HSET"
            | "HDEL"
            | "SADD"
            | "SREM"
            | "ZADD"
            | "ZREM"
            | "ZREMRANGEBYSCORE"
            | "EVAL"
    );
    if !mutates {
        return TypedMobilitySyncScope::None;
    }
    if command.name == "EVAL"
        && command.args.first().is_some_and(|script| {
            script.contains("fn-knock:eval:collect-mobility-session-whitelist:v1")
        })
    {
        return TypedMobilitySyncScope::None;
    }
    match command.name.as_str() {
        "DEL" => TypedMobilitySyncScope::from_keys(command.args.iter().cloned()),
        "EVAL" => {
            let Some(key_count) = command
                .args
                .get(1)
                .and_then(|value| value.parse::<usize>().ok())
            else {
                return if command.args.first().is_some_and(|script| {
                    script.contains("fn_knock:session:")
                        || script.contains("fn_knock:auth_mobility:")
                }) {
                    TypedMobilitySyncScope::All
                } else {
                    TypedMobilitySyncScope::None
                };
            };
            TypedMobilitySyncScope::from_keys(command.args.iter().skip(2).take(key_count).cloned())
        }
        _ => command
            .args
            .first()
            .map(|key| TypedMobilitySyncScope::from_key(key))
            .unwrap_or_default(),
    }
}

pub(super) fn sync_typed_mobility_tx(
    tx: &rusqlite::Transaction<'_>,
    scope: TypedMobilitySyncScope,
) -> RedisResult<()> {
    // This scope began with mobility aggregates and now also carries the
    // security-sensitive login-backoff shadow. Both repositories filter the
    // exact keys they own, while `All` is reserved for keyspace replacement.
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'whitelist_auto_owner_mappings'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_whitelist_runtime::TypedWhitelistRuntimeRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_whitelist_runtime::TypedWhitelistRuntimeRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'subdomain_rule_grants'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_subdomain_grant::TypedSubdomainGrantRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_subdomain_grant::TypedSubdomainGrantRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'fnos_share_runtime_capabilities'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_fnos_share::TypedFnosShareRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_fnos_share::TypedFnosShareRepository::rebuild_from_legacy_tx(
                    tx,
                )?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'hmac_replay_nonces'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_hmac_nonce::TypedHmacNonceRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_hmac_nonce::TypedHmacNonceRepository::rebuild_from_legacy_tx(
                    tx,
                )?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'mobility_session_aggregates'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_mobility::TypedMobilityRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_mobility::TypedMobilityRepository::rebuild_from_legacy_tx(
                    tx,
                )?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'system_event_dedupe_leases'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_event_dedupe::TypedEventDedupeRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_event_dedupe::TypedEventDedupeRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'login_backoff_attempts'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_login_backoff::TypedLoginBackoffRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_login_backoff::TypedLoginBackoffRepository::rebuild_from_legacy_tx(
                    tx,
                )?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'docker_admin_session_documents'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_docker_admin::TypedDockerAdminRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_docker_admin::TypedDockerAdminRepository::rebuild_from_legacy_tx(
                    tx,
                )?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'subdomain_rule_rate_limit_counters'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_subdomain_rate_limit::TypedSubdomainRateLimitRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_subdomain_rate_limit::TypedSubdomainRateLimitRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'wol_wake_cooldowns'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_wol_cooldown::TypedWolCooldownRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_wol_cooldown::TypedWolCooldownRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'notification_runtime_leases'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_notification_runtime::TypedNotificationRuntimeRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_notification_runtime::TypedNotificationRuntimeRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'passkey_runtime_capabilities'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_passkey_runtime::TypedPasskeyRuntimeRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_passkey_runtime::TypedPasskeyRuntimeRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    if !matches!(scope, TypedMobilitySyncScope::None)
        && tx.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master
               WHERE type = 'table' AND name = 'identity_runtime_aggregates'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        match &scope {
            TypedMobilitySyncScope::None => {}
            TypedMobilitySyncScope::Keys(keys) => {
                crate::storage::typed_identity_runtime::TypedIdentityRuntimeRepository::reconcile_legacy_keys_tx(
                    tx,
                    &keys.iter().cloned().collect::<Vec<_>>(),
                )?;
            }
            TypedMobilitySyncScope::All => {
                crate::storage::typed_identity_runtime::TypedIdentityRuntimeRepository::rebuild_from_legacy_tx(tx)?;
            }
        }
    }
    Ok(())
}
