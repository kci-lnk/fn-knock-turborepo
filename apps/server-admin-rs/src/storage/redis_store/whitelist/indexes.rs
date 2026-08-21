use super::*;

pub(in crate::storage::redis_store) fn queue_whitelist_indexes(
    pipe: &mut redis::Pipeline,
    record: &WhitelistRecord,
) {
    match record.target_type() {
        "cidr" => {
            pipe.sadd(WHITELIST_CIDR_RECORDS, &record.id).ignore();
        }
        "cname" => {
            for target in record.concrete_targets() {
                if target.target_type == "ip" {
                    pipe.sadd(WHITELIST_IPS, &target.target).ignore();
                    pipe.sadd(whitelist_ip_records_key(&target.target), &record.id)
                        .ignore();
                }
            }
        }
        _ => {
            pipe.sadd(WHITELIST_IPS, &record.ip).ignore();
            pipe.sadd(whitelist_ip_records_key(&record.ip), &record.id)
                .ignore();
        }
    }
}

pub(in crate::storage::redis_store) fn queue_remove_whitelist_indexes(
    pipe: &mut redis::Pipeline,
    record: &WhitelistRecord,
) {
    match record.target_type() {
        "cidr" => {
            pipe.srem(WHITELIST_CIDR_RECORDS, &record.id).ignore();
        }
        "cname" => {
            for target in record.concrete_targets() {
                if target.target_type == "ip" {
                    pipe.srem(whitelist_ip_records_key(&target.target), &record.id)
                        .ignore();
                }
            }
        }
        _ => {
            pipe.srem(whitelist_ip_records_key(&record.ip), &record.id)
                .ignore();
        }
    }
}

pub(in crate::storage::redis_store) fn whitelist_stale_ip_index_targets(
    record: &WhitelistRecord,
) -> Vec<String> {
    let mut targets = Vec::new();
    for target in record.concrete_targets() {
        if target.target_type != "ip" || targets.iter().any(|value| value == &target.target) {
            continue;
        }
        targets.push(target.target);
    }
    targets
}
pub(in crate::storage::redis_store) fn unique_concrete_targets(
    targets: &[WhitelistConcreteTarget],
) -> Vec<WhitelistConcreteTarget> {
    let mut unique = Vec::new();
    for target in targets {
        if unique.iter().any(|candidate: &WhitelistConcreteTarget| {
            candidate.target == target.target && candidate.target_type == target.target_type
        }) {
            continue;
        }
        unique.push(target.clone());
    }
    unique
}

pub(in crate::storage::redis_store) fn unique_non_empty_strings(values: &[String]) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() || unique.iter().any(|item: &String| item == normalized) {
            continue;
        }
        unique.push(normalized.to_string());
    }
    unique
}
