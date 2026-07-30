use super::*;

impl Store {
    pub async fn get_whitelist_record(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.hget(WHITELIST_RECORDS, id).await?;
        Ok(raw.and_then(|value| deserialize_whitelist_record(&value)))
    }

    pub async fn list_whitelist_records(
        &self,
    ) -> crate::storage::StorageResult<Vec<WhitelistRecord>> {
        let mut conn = self.conn();
        let ids: Vec<String> = conn.zrevrange(WHITELIST_RECORD_ORDER, 0, -1).await?;
        if ids.is_empty() {
            return self.rebuild_whitelist_indexes().await;
        }

        let raws: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(WHITELIST_RECORDS)
            .arg(ids.clone())
            .query_async(&mut conn)
            .await?;
        let mut records = Vec::new();
        let mut stale_ids = Vec::new();
        let mut stale_ip_targets = BTreeSet::new();
        for (id, raw) in ids.into_iter().zip(raws) {
            let Some(raw) = raw else {
                stale_ids.push(id);
                continue;
            };
            let Some(record) = deserialize_whitelist_record(&raw) else {
                stale_ids.push(id);
                continue;
            };
            if record.is_active() {
                records.push(record);
            } else if record.status == "pending" {
                // Pending login/mobility grants are intentionally invisible to
                // authorization compilers, but their indexes must remain in
                // place so the live-session transaction can promote them.
                continue;
            } else {
                for target in whitelist_stale_ip_index_targets(&record) {
                    stale_ip_targets.insert(target);
                }
                stale_ids.push(id);
            }
        }
        if !stale_ids.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(WHITELIST_RECORD_ORDER, stale_ids.clone())
                .ignore();
            pipe.zrem(WHITELIST_EXPIRY, stale_ids.clone()).ignore();
            pipe.srem(WHITELIST_CIDR_RECORDS, stale_ids.clone())
                .ignore();
            for ip in stale_ip_targets {
                pipe.srem(whitelist_ip_records_key(&ip), stale_ids.clone())
                    .ignore();
            }
            let _: () = pipe.query_async(&mut conn).await?;
        }
        records.sort_by_key(|record| std::cmp::Reverse(record.created_at));
        Ok(records)
    }

    pub async fn insert_whitelist_record(
        &self,
        record: &WhitelistRecord,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_RECORDS,
            &record.id,
            serde_json::to_string(record).unwrap_or_default(),
        )
        .ignore();
        pipe.zadd(WHITELIST_RECORD_ORDER, &record.id, record.created_at)
            .ignore();
        if let Some(expire_at) = record.expire_at {
            pipe.zadd(WHITELIST_EXPIRY, &record.id, expire_at).ignore();
        }
        queue_whitelist_indexes(&mut pipe, record);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn replace_whitelist_record(
        &self,
        previous: &WhitelistRecord,
        next: &WhitelistRecord,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_RECORDS,
            &next.id,
            serde_json::to_string(next).unwrap_or_default(),
        )
        .ignore();
        if let Some(expire_at) = next.expire_at {
            pipe.zadd(WHITELIST_EXPIRY, &next.id, expire_at).ignore();
        } else {
            pipe.zrem(WHITELIST_EXPIRY, &next.id).ignore();
        }
        queue_remove_whitelist_indexes(&mut pipe, previous);
        queue_whitelist_indexes(&mut pipe, next);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn delete_whitelist_record(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        let Some(record) = self.get_whitelist_record(id).await? else {
            return Ok(None);
        };
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hdel(WHITELIST_RECORDS, id).ignore();
        pipe.hdel(WHITELIST_DELETED, id).ignore();
        pipe.zrem(WHITELIST_RECORD_ORDER, id).ignore();
        pipe.zrem(WHITELIST_EXPIRY, id).ignore();
        queue_remove_whitelist_indexes(&mut pipe, &record);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn expire_whitelist_record(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        let Some(record) = self.get_whitelist_record(id).await? else {
            return Ok(None);
        };
        if !record.is_active() {
            return Ok(None);
        }
        let mut next = record.clone();
        next.status = "expired".to_string();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_RECORDS,
            id,
            serde_json::to_string(&next).unwrap_or_default(),
        )
        .ignore();
        pipe.zrem(WHITELIST_RECORD_ORDER, id).ignore();
        pipe.zrem(WHITELIST_EXPIRY, id).ignore();
        queue_remove_whitelist_indexes(&mut pipe, &record);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn update_whitelist_comment(
        &self,
        id: &str,
        comment: String,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        let Some(mut record) = self.get_whitelist_record(id).await? else {
            return Ok(None);
        };
        record.comment = Some(comment);
        let mut conn = self.conn();
        let _: () = conn
            .hset(
                WHITELIST_RECORDS,
                id,
                serde_json::to_string(&record).unwrap_or_default(),
            )
            .await?;
        Ok(Some(record))
    }

    pub async fn find_whitelist_records_by_target(
        &self,
        target: &str,
        target_type: &str,
        source: Option<&str>,
    ) -> crate::storage::StorageResult<Vec<WhitelistRecord>> {
        let records = self.list_whitelist_records().await?;
        let mut matched = records
            .into_iter()
            .filter(|record| {
                if let Some(source) = source
                    && record.source != source
                {
                    return false;
                }
                match target_type {
                    "cidr" => record.target_type() == "cidr" && record.ip == target,
                    "cname" => record.target_type() == "cname" && record.ip == target,
                    _ => record.target_type() == "ip" && record.ip == target,
                }
            })
            .collect::<Vec<_>>();
        matched.sort_by_key(|record| std::cmp::Reverse(record.created_at));
        Ok(matched)
    }

    pub async fn get_whitelist_region_group(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRegionGroupRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.hget(WHITELIST_REGION_GROUP_RECORDS, id).await?;
        Ok(raw.and_then(|value| deserialize_whitelist_region_group(&value)))
    }

    pub async fn migrate_whitelist_region_groups_to_ipsets(
        &self,
    ) -> crate::storage::StorageResult<Vec<WhitelistRegionGroupRecord>> {
        let mut conn = self.conn();
        let all: HashMap<String, String> = conn.hgetall(WHITELIST_REGION_GROUP_RECORDS).await?;
        let now = chrono_like_now_seconds();
        let mut active = Vec::new();
        let mut rewritten = Vec::new();
        for (field, raw) in all {
            let Some(mut record) = deserialize_whitelist_region_group(&raw) else {
                return Err(crate::storage::storage_error(format!(
                    "whitelist region group {field} is malformed"
                )));
            };
            if record.id != field {
                return Err(crate::storage::storage_error(format!(
                    "whitelist region group field {field} does not match record id {}",
                    record.id
                )));
            }
            let is_active =
                record.is_active() && record.expire_at.is_none_or(|expire_at| expire_at > now);
            if is_active {
                let mut policy = if let Some(value) = record.policy.as_ref() {
                    crate::cidr::CompiledIpSet::from_transport_value(value).map_err(|error| {
                        crate::storage::storage_error(format!(
                            "whitelist region group {} policy is invalid: {error}",
                            record.id
                        ))
                    })?
                } else {
                    crate::cidr::compile_ip_set(&record.cidrs).map_err(|error| {
                        crate::storage::storage_error(format!(
                            "whitelist region group {} CIDRs are invalid: {error}",
                            record.id
                        ))
                    })?
                }
                .into_current_format();
                if !record.policy_id.trim().is_empty() && record.policy_id != policy.id {
                    return Err(crate::storage::storage_error(format!(
                        "whitelist region group {} policy reference mismatch",
                        record.id
                    )));
                }
                if record.source_cidr_count > 0 {
                    policy.source_cidr_count = record.source_cidr_count;
                }
                record.source_cidr_count = policy.source_cidr_count;
                record.range_count = policy.range_count();
                record.policy_id = policy.id.clone();
                record.policy = Some(policy.to_transport_value());
                record.cidrs.clear();
                active.push(record.clone());
            } else {
                if record.status == "active" {
                    record.status = "expired".to_string();
                    record.updated_at = now;
                }
                record.cidrs.clear();
                record.policy_id.clear();
                record.policy = None;
                record.source_cidr_count = 0;
                record.range_count = 0;
            }
            rewritten.push(record);
        }

        let mut pipe = redis::pipe();
        pipe.del(WHITELIST_REGION_GROUP_ORDER).ignore();
        pipe.del(WHITELIST_REGION_GROUP_EXPIRY).ignore();
        for record in &rewritten {
            pipe.hset(
                WHITELIST_REGION_GROUP_RECORDS,
                &record.id,
                serde_json::to_string(record)?,
            )
            .ignore();
            if record.is_active() {
                pipe.zadd(WHITELIST_REGION_GROUP_ORDER, &record.id, record.created_at)
                    .ignore();
                if let Some(expire_at) = record.expire_at {
                    pipe.zadd(WHITELIST_REGION_GROUP_EXPIRY, &record.id, expire_at)
                        .ignore();
                }
            }
        }
        let _: () = pipe.query_async(&mut conn).await?;
        active.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(active)
    }

    pub async fn list_whitelist_region_groups(
        &self,
    ) -> crate::storage::StorageResult<Vec<WhitelistRegionGroupRecord>> {
        let mut conn = self.conn();
        let ids: Vec<String> = conn.zrevrange(WHITELIST_REGION_GROUP_ORDER, 0, -1).await?;
        let mut stale_ids = Vec::new();
        let mut records = if ids.is_empty() {
            Vec::new()
        } else {
            let raws: Vec<Option<String>> = redis::cmd("HMGET")
                .arg(WHITELIST_REGION_GROUP_RECORDS)
                .arg(ids.clone())
                .query_async(&mut conn)
                .await?;
            let mut records = Vec::new();
            for (id, raw) in ids.into_iter().zip(raws) {
                let Some(raw) = raw else {
                    stale_ids.push(id);
                    continue;
                };
                let Some(record) = deserialize_whitelist_region_group(&raw) else {
                    stale_ids.push(id);
                    continue;
                };
                if record.is_active() {
                    records.push(record);
                } else {
                    stale_ids.push(id);
                }
            }
            records
        };
        if !stale_ids.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(WHITELIST_REGION_GROUP_ORDER, stale_ids.clone())
                .ignore();
            pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, stale_ids).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }
        records.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(records)
    }

    pub async fn insert_whitelist_region_group(
        &self,
        record: &WhitelistRegionGroupRecord,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_REGION_GROUP_RECORDS,
            &record.id,
            serde_json::to_string(record).unwrap_or_default(),
        )
        .ignore();
        pipe.zadd(WHITELIST_REGION_GROUP_ORDER, &record.id, record.created_at)
            .ignore();
        if let Some(expire_at) = record.expire_at {
            pipe.zadd(WHITELIST_REGION_GROUP_EXPIRY, &record.id, expire_at)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn delete_whitelist_region_group(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRegionGroupRecord>> {
        let Some(record) = self.get_whitelist_region_group(id).await? else {
            return Ok(None);
        };
        if !record.is_active() {
            return Ok(None);
        }
        let mut next = record.clone();
        next.status = "deleted".to_string();
        next.updated_at = chrono_like_now_seconds();
        next.cidrs.clear();
        next.policy_id.clear();
        next.policy = None;
        next.source_cidr_count = 0;
        next.range_count = 0;
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_REGION_GROUP_RECORDS,
            id,
            serde_json::to_string(&next).unwrap_or_default(),
        )
        .ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_ORDER, id).ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, id).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn expire_whitelist_region_group(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRegionGroupRecord>> {
        let Some(record) = self.get_whitelist_region_group(id).await? else {
            return Ok(None);
        };
        if !record.is_active() {
            return Ok(None);
        }
        let mut next = record.clone();
        next.status = "expired".to_string();
        next.updated_at = chrono_like_now_seconds();
        next.cidrs.clear();
        next.policy_id.clear();
        next.policy = None;
        next.source_cidr_count = 0;
        next.range_count = 0;
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_REGION_GROUP_RECORDS,
            id,
            serde_json::to_string(&next).unwrap_or_default(),
        )
        .ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_ORDER, id).ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, id).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn cleanup_whitelist_concrete_targets(
        &self,
        targets: &[WhitelistConcreteTarget],
    ) -> crate::storage::StorageResult<Vec<WhitelistConcreteTarget>> {
        let active_records = self.list_whitelist_records().await?;
        let active_region_policies = self
            .list_whitelist_region_groups()
            .await?
            .into_iter()
            .filter_map(|record| record.policy())
            .collect::<Vec<_>>();
        let active_region_policy = crate::cidr::union_ip_sets(active_region_policies.iter());
        let mut removed = Vec::new();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for target in unique_concrete_targets(targets) {
            let still_active = active_records.iter().any(|record| {
                record.concrete_targets().iter().any(|candidate| {
                    candidate.target == target.target && candidate.target_type == target.target_type
                })
            });
            if still_active {
                continue;
            }
            if target.target_type == "cidr" && active_region_policy.contains_cidr(&target.target) {
                continue;
            }

            if target.target_type == "cidr" {
                pipe.srem(WHITELIST_CIDR_RECORDS, &target.record_id)
                    .ignore();
            } else {
                pipe.srem(WHITELIST_IPS, &target.target).ignore();
                pipe.del(whitelist_ip_records_key(&target.target)).ignore();
            }
            removed.push(target);
        }

        if !removed.is_empty() {
            let _: () = pipe.query_async(&mut conn).await?;
        }
        Ok(removed)
    }

    pub async fn list_whitelist_active_concrete_targets(
        &self,
    ) -> crate::storage::StorageResult<Vec<WhitelistConcreteTarget>> {
        let now = chrono_like_now_seconds();
        let mut targets = Vec::new();
        for record in self.list_whitelist_records().await? {
            if !record.is_active() {
                continue;
            }
            if record.expire_at.is_some_and(|expire_at| expire_at <= now) {
                continue;
            }
            targets.extend(record.concrete_targets());
        }
        Ok(targets)
    }

    pub async fn save_gateway_trusted_client_ips_runtime(
        &self,
        runtime: &Value,
    ) -> crate::storage::StorageResult<()> {
        self.set_json_value(GATEWAY_TRUSTED_CLIENT_IPS_RUNTIME, runtime)
            .await
    }

    pub async fn save_reverse_proxy_trusted_ips_runtime(
        &self,
        runtime: &Value,
    ) -> crate::storage::StorageResult<()> {
        self.set_json_value(REVERSE_PROXY_TRUSTED_IPS_RUNTIME, runtime)
            .await
    }

    async fn rebuild_whitelist_indexes(
        &self,
    ) -> crate::storage::StorageResult<Vec<WhitelistRecord>> {
        let mut conn = self.conn();
        let all: HashMap<String, String> = conn.hgetall(WHITELIST_RECORDS).await?;
        let existing_ips: Vec<String> = conn.smembers(WHITELIST_IPS).await.unwrap_or_default();
        let mut records = Vec::new();

        for raw in all.values() {
            let Some(record) = deserialize_whitelist_record(raw) else {
                continue;
            };
            if record.is_active() {
                records.push(record);
            }
        }
        records.sort_by_key(|record| std::cmp::Reverse(record.created_at));

        let mut pipe = redis::pipe();
        pipe.del(WHITELIST_RECORD_ORDER).ignore();
        pipe.del(WHITELIST_EXPIRY).ignore();
        pipe.del(WHITELIST_IPS).ignore();
        pipe.del(WHITELIST_CIDR_RECORDS).ignore();
        for ip in existing_ips {
            pipe.del(whitelist_ip_records_key(&ip)).ignore();
        }
        for record in &records {
            pipe.zadd(WHITELIST_RECORD_ORDER, &record.id, record.created_at)
                .ignore();
            if let Some(expire_at) = record.expire_at {
                pipe.zadd(WHITELIST_EXPIRY, &record.id, expire_at).ignore();
            }
            queue_whitelist_indexes(&mut pipe, record);
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(records)
    }
}
