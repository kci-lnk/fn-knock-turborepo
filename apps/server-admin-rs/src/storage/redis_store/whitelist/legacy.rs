pub(in crate::storage::redis_store) const WHITELIST_RECORDS: &str = "fn_knock:whitelist:records";
pub(in crate::storage::redis_store) const WHITELIST_RECORD_ORDER: &str =
    "fn_knock:whitelist:record_order";
pub(in crate::storage::redis_store) const WHITELIST_EXPIRY: &str = "fn_knock:whitelist:expiry";
pub(in crate::storage::redis_store) const WHITELIST_IPS: &str = "fn_knock:whitelist:ips";
pub(in crate::storage::redis_store) const WHITELIST_CIDR_RECORDS: &str =
    "fn_knock:whitelist:cidr_records";
pub(in crate::storage::redis_store) const WHITELIST_DELETED: &str = "fn_knock:whitelist:deleted";
pub(in crate::storage::redis_store) const WHITELIST_REGION_GROUP_RECORDS: &str =
    "fn_knock:whitelist:region_groups:records";
pub(in crate::storage::redis_store) const WHITELIST_REGION_GROUP_ORDER: &str =
    "fn_knock:whitelist:region_groups:order";
pub(in crate::storage::redis_store) const WHITELIST_REGION_GROUP_EXPIRY: &str =
    "fn_knock:whitelist:region_groups:expiry";
pub(in crate::storage::redis_store) fn default_whitelist_target_type() -> String {
    "ip".to_string()
}

pub(in crate::storage::redis_store) fn default_whitelist_source() -> String {
    "manual".to_string()
}

pub(in crate::storage::redis_store) fn default_whitelist_status() -> String {
    "active".to_string()
}

pub(in crate::storage::redis_store) fn whitelist_ip_records_key(ip: &str) -> String {
    format!("fn_knock:whitelist:ip_records:{ip}")
}
