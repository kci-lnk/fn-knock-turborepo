use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
    },
};

use crate::storage::redis_compat as redis;
use arc_swap::ArcSwap;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ipnet::IpNet;
use redis::{
    ConnectionManager,
    streams::{StreamRangeReply, StreamReadOptions, StreamReadReply},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::storage::typed_config::TypedConfigRepository;
use crate::storage::typed_docker_admin::TypedDockerAdminRepository;
use crate::storage::typed_event_dedupe::TypedEventDedupeRepository;
use crate::storage::typed_events::TypedEventRepository;
use crate::storage::typed_fnos_share::TypedFnosShareRepository;
use crate::storage::typed_hmac_nonce::TypedHmacNonceRepository;
use crate::storage::typed_identity_runtime::TypedIdentityRuntimeRepository;
use crate::storage::typed_login_backoff::TypedLoginBackoffRepository;
use crate::storage::typed_mobility::TypedMobilityRepository;
use crate::storage::typed_notification_runtime::TypedNotificationRuntimeRepository;
use crate::storage::typed_notifications::TypedNotificationRepository;
use crate::storage::typed_passkey_runtime::TypedPasskeyRuntimeRepository;
use crate::storage::typed_subdomain_grant::TypedSubdomainGrantRepository;
use crate::storage::typed_subdomain_rate_limit::TypedSubdomainRateLimitRepository;
use crate::storage::typed_whitelist::{TypedWhitelistDocument, TypedWhitelistRepository};
use crate::storage::typed_whitelist_runtime::TypedWhitelistRuntimeRepository;
use crate::storage::typed_wol_cooldown::TypedWolCooldownRepository;
use crate::{
    auth_mobility_keys::{
        active_ip_details_key as auth_mobility_active_ip_details_key,
        active_ip_zset_key as auth_mobility_active_ip_zset_key,
        binding_key as auth_mobility_binding_key,
        session_index_key as auth_mobility_session_index_key,
        session_pending_whitelist_key as auth_mobility_session_pending_whitelist_key,
        subject_hash as auth_mobility_subject_hash, summary_key as auth_mobility_summary_key,
        timeline_key as auth_mobility_timeline_key,
        whitelist_owner_key as auth_mobility_whitelist_owner_key,
    },
    http_utils::normalize_ip,
    time_utils::{iso_after_seconds, now_iso},
};

mod auth;
mod config;
mod core;
mod discovery;
mod docker_admin;
mod events;
mod facade;
mod lifecycle;
mod notifications;
mod session_factory;
mod traffic;
mod types;
mod waf_logs;
mod whitelist;

pub(crate) use auth::compat::{normalize_totp_access_scopes, normalize_totp_subdomain_access};
pub use config::default_config;
use core::node_compat::{js_finite_number, js_string};
pub(crate) use core::node_locale_compare_ordering;
use core::runtime_keys::{GATEWAY_TRUSTED_CLIENT_IPS_RUNTIME, REVERSE_PROXY_TRUSTED_IPS_RUNTIME};
pub(crate) use core::{LdapBindingClaim, OidcBindingClaim, OwnedBindingDelete, OwnedBindingUpdate};
pub(crate) use events::compat::system_event_matches_filters;
pub use facade::Store;
pub(crate) use facade::{
    CONFIG_GENERATION_MARKER, referenced_host_ipset_policy_ids, strip_internal_config_metadata,
};
use facade::{
    CONFIG_KEY, HOST_MAPPINGS_GENERATION_KEY, LoginBackoffAttemptState, ShadowTracker,
    TypedConfigShadowStatus, TypedRepositories,
};
use notifications::{
    NOTIFICATION_DELIVERIES_READY_KEY, NOTIFICATION_DELIVERY_QUEUE_TTL_SECONDS,
    NOTIFICATION_RUNTIME_COOLDOWN_PREFIX, NOTIFICATION_RUNTIME_LAST_STREAM_KEY,
    NOTIFICATION_RUNTIME_LOCK_PREFIX, NOTIFICATION_RUNTIME_WINDOW_PREFIX,
    notification_cooldown_key, notification_runtime_lock_key, notification_window_key,
};
#[allow(unused_imports)]
pub use session_factory::new_login_session;
use traffic::chrono_like_now_seconds;
pub use types::*;
use whitelist::*;

#[cfg(test)]
use auth::{
    compat::{login_backoff_status_from_raw, normalize_totp_credentials_value},
    legacy::login_backoff_key,
};
#[cfg(test)]
use docker_admin::{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX, DOCKER_ADMIN_SESSION_PREFIX};
#[cfg(test)]
use events::*;
#[cfg(test)]
use traffic::{compute_counter_delta, parse_traffic_points, traffic_scope_segment};
#[cfg(test)]
use waf_logs::{waf_log_dates_for_range, waf_log_stats_key};

#[cfg(test)]
mod tests;
