pub(crate) const AUTH_MOBILITY_PREFIX: &str = "fn_knock:auth_mobility";

pub(crate) fn active_ip_details_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:active_ip_details:{session_id}")
}

pub(crate) fn active_ip_zset_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:active_ips:{session_id}")
}

pub(crate) fn binding_key(subject_type: &str, subject_hash: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:binding:{subject_type}:{subject_hash}")
}

pub(crate) fn session_index_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:session:{session_id}")
}

pub(crate) fn session_mutation_lock_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:session_mutation_lock:{session_id}")
}

pub(crate) fn session_pending_whitelist_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:session_pending_whitelist:{session_id}")
}

pub(crate) fn summary_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:summary:{session_id}")
}

pub(crate) fn timeline_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:timeline:{session_id}")
}

pub(crate) fn whitelist_owner_key(whitelist_record_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:whitelist:{whitelist_record_id}:session")
}

pub(crate) fn subject_hash(subject_type: &str, subject_key: &str) -> String {
    crate::crypto_utils::sha256_hex_str(&format!("{subject_type}:{subject_key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_auth_mobility_storage_keys() {
        assert_eq!(
            timeline_key("session-1"),
            "fn_knock:auth_mobility:timeline:session-1"
        );
        assert_eq!(
            summary_key("session-1"),
            "fn_knock:auth_mobility:summary:session-1"
        );
        assert_eq!(
            whitelist_owner_key("record-1"),
            "fn_knock:auth_mobility:whitelist:record-1:session"
        );
    }

    #[test]
    fn hashes_auth_mobility_subjects_with_existing_contract() {
        assert_eq!(
            subject_hash("fnos-token", "secret-token"),
            crate::crypto_utils::sha256_hex_str("fnos-token:secret-token")
        );
    }
}
