use super::*;

const WHITELIST_MUTATION_MAX_RETRIES: usize = 8;

enum TypedWhitelistMutation {
    Upsert(TypedWhitelistDocument),
    Delete {
        kind: &'static str,
        id: String,
    },
    ReplaceKind {
        kind: &'static str,
        documents: Vec<TypedWhitelistDocument>,
    },
}

fn typed_whitelist_record(
    record: &WhitelistRecord,
) -> crate::storage::StorageResult<TypedWhitelistDocument> {
    Ok(TypedWhitelistDocument {
        kind: "record",
        id: record.id.clone(),
        document_json: serde_json::to_string(record)?,
        sort_score: record.created_at,
        expires_at: record.expire_at,
        status: record.status.clone(),
    })
}

fn typed_whitelist_region(
    record: &WhitelistRegionGroupRecord,
) -> crate::storage::StorageResult<TypedWhitelistDocument> {
    Ok(TypedWhitelistDocument {
        kind: "region",
        id: record.id.clone(),
        document_json: serde_json::to_string(record)?,
        sort_score: record.created_at,
        expires_at: record.expire_at,
        status: record.status.clone(),
    })
}

fn whitelist_record_from_typed(
    document: TypedWhitelistDocument,
) -> crate::storage::StorageResult<WhitelistRecord> {
    let record = deserialize_whitelist_record(&document.document_json).ok_or_else(|| {
        crate::storage::storage_error(format!(
            "typed whitelist record {} is malformed",
            document.id
        ))
    })?;
    if record.id != document.id
        || record.created_at != document.sort_score
        || record.expire_at != document.expires_at
        || record.status != document.status
    {
        return Err(crate::storage::storage_error(format!(
            "typed whitelist record {} metadata mismatch",
            document.id
        )));
    }
    Ok(record)
}

fn whitelist_region_from_typed(
    document: TypedWhitelistDocument,
) -> crate::storage::StorageResult<WhitelistRegionGroupRecord> {
    let record = deserialize_whitelist_region_group(&document.document_json).ok_or_else(|| {
        crate::storage::storage_error(format!(
            "typed whitelist region {} is malformed",
            document.id
        ))
    })?;
    if record.id != document.id
        || record.created_at != document.sort_score
        || record.expire_at != document.expires_at
        || record.status != document.status
    {
        return Err(crate::storage::storage_error(format!(
            "typed whitelist region {} metadata mismatch",
            document.id
        )));
    }
    Ok(record)
}

impl Store {
    pub(crate) async fn rebuild_typed_whitelist_from_legacy(
        &self,
    ) -> crate::storage::StorageResult<()> {
        self.conn()
            .call(|conn| {
                let tx = conn.transaction_with_behavior(
                    tokio_rusqlite::rusqlite::TransactionBehavior::Immediate,
                )?;
                let records = redis::hash_entries_in_transaction(&tx, WHITELIST_RECORDS)?;
                let regions =
                    redis::hash_entries_in_transaction(&tx, WHITELIST_REGION_GROUP_RECORDS)?;
                let mut documents = Vec::with_capacity(records.len() + regions.len());
                for (field, raw) in records {
                    let Some(record) = deserialize_whitelist_record(&raw) else {
                        continue;
                    };
                    if record.id == field {
                        documents.push(typed_whitelist_record(&record)?);
                    }
                }
                for (field, raw) in regions {
                    let Some(region) = deserialize_whitelist_region_group(&raw) else {
                        continue;
                    };
                    if region.id == field {
                        documents.push(typed_whitelist_region(&region)?);
                    }
                }
                TypedWhitelistRepository::replace_all_tx(&tx, &documents)?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    fn observe_typed_whitelist_shadow_healthy(&self) {
        if !self
            .typed_whitelist_shadow_healthy
            .swap(true, AtomicOrdering::AcqRel)
        {
            tracing::info!("typed whitelist document comparison recovered");
        }
    }

    fn observe_typed_whitelist_shadow_failure(&self, reason: &str) {
        self.typed_whitelist_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        if self
            .typed_whitelist_shadow_healthy
            .swap(false, AtomicOrdering::AcqRel)
        {
            tracing::warn!(%reason, "typed whitelist document comparison failed");
        }
    }

    #[cfg(test)]
    pub(crate) fn typed_whitelist_shadow_mismatch_count(&self) -> u64 {
        self.typed_whitelist_shadow_mismatches
            .load(AtomicOrdering::Acquire)
    }

    pub async fn get_whitelist_record(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        let typed = self
            .typed_whitelist
            .load_one("record", id)
            .await
            .and_then(|document| document.map(whitelist_record_from_typed).transpose());
        let legacy = self.get_whitelist_record_legacy(id).await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => {
                self.observe_typed_whitelist_shadow_healthy();
                Ok(typed)
            }
            (Ok(_), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(
                    "typed whitelist record differs from legacy keyspace",
                );
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Err(error), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "typed whitelist record read failed; using legacy fallback: {error}"
                ));
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Ok(_), Err(error)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "legacy whitelist record comparison failed; refusing typed-only authorization data: {error}"
                ));
                Err(error)
            }
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy whitelist record reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn get_whitelist_record_legacy(
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
        let typed = self
            .typed_whitelist
            .load_kind("record")
            .await
            .and_then(|documents| {
                let mut records = documents
                    .into_iter()
                    .map(whitelist_record_from_typed)
                    .collect::<crate::storage::StorageResult<Vec<_>>>()?;
                records.retain(WhitelistRecord::is_active);
                records.sort_by_key(|record| std::cmp::Reverse(record.created_at));
                Ok(records)
            });
        let legacy = self.list_whitelist_records_legacy().await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => {
                self.observe_typed_whitelist_shadow_healthy();
                Ok(typed)
            }
            (Ok(_), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(
                    "typed whitelist record list differs from legacy keyspace",
                );
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Err(error), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "typed whitelist record list failed; using legacy fallback: {error}"
                ));
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Ok(_), Err(error)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "legacy whitelist list comparison failed; refusing typed-only authorization data: {error}"
                ));
                Err(error)
            }
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy whitelist list reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn list_whitelist_records_legacy(
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
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_RECORDS,
            &record.id,
            serde_json::to_string(record)?,
        )
        .ignore();
        pipe.zadd(WHITELIST_RECORD_ORDER, &record.id, record.created_at)
            .ignore();
        if let Some(expire_at) = record.expire_at {
            pipe.zadd(WHITELIST_EXPIRY, &record.id, expire_at).ignore();
        }
        queue_whitelist_indexes(&mut pipe, record);
        self.execute_typed_whitelist_pipeline(
            TypedWhitelistMutation::Upsert(typed_whitelist_record(record)?),
            pipe,
        )
        .await
    }

    pub async fn replace_whitelist_record(
        &self,
        previous: &WhitelistRecord,
        next: &WhitelistRecord,
    ) -> crate::storage::StorageResult<()> {
        if self.try_replace_whitelist_record(previous, next).await? {
            return Ok(());
        }
        Err(crate::storage::storage_error(format!(
            "whitelist record {} changed concurrently",
            previous.id
        )))
    }

    async fn try_replace_whitelist_record(
        &self,
        previous: &WhitelistRecord,
        next: &WhitelistRecord,
    ) -> crate::storage::StorageResult<bool> {
        if previous.id != next.id {
            return Err(crate::storage::storage_error(
                "whitelist record id cannot change during replacement",
            ));
        }
        let mut pipe = redis::pipe();
        pipe.hset(WHITELIST_RECORDS, &next.id, serde_json::to_string(next)?)
            .ignore();
        if let Some(expire_at) = next.expire_at {
            pipe.zadd(WHITELIST_EXPIRY, &next.id, expire_at).ignore();
        } else {
            pipe.zrem(WHITELIST_EXPIRY, &next.id).ignore();
        }
        queue_remove_whitelist_indexes(&mut pipe, previous);
        queue_whitelist_indexes(&mut pipe, next);
        self.execute_whitelist_record_pipeline_if_current(
            previous,
            TypedWhitelistMutation::Upsert(typed_whitelist_record(next)?),
            pipe,
        )
        .await
    }

    pub async fn delete_whitelist_record(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        for _ in 0..WHITELIST_MUTATION_MAX_RETRIES {
            let Some(record) = self.get_whitelist_record(id).await? else {
                return Ok(None);
            };
            let mut pipe = redis::pipe();
            pipe.hdel(WHITELIST_RECORDS, id).ignore();
            pipe.hdel(WHITELIST_DELETED, id).ignore();
            pipe.zrem(WHITELIST_RECORD_ORDER, id).ignore();
            pipe.zrem(WHITELIST_EXPIRY, id).ignore();
            queue_remove_whitelist_indexes(&mut pipe, &record);
            if self
                .execute_whitelist_record_pipeline_if_current(
                    &record,
                    TypedWhitelistMutation::Delete {
                        kind: "record",
                        id: id.to_string(),
                    },
                    pipe,
                )
                .await?
            {
                return Ok(Some(record));
            }
        }
        Err(crate::storage::storage_error(format!(
            "whitelist record {id} kept changing during deletion"
        )))
    }

    pub async fn expire_whitelist_record(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        for _ in 0..WHITELIST_MUTATION_MAX_RETRIES {
            let Some(record) = self.get_whitelist_record(id).await? else {
                return Ok(None);
            };
            if !record.is_active() {
                return Ok(None);
            }
            let mut next = record.clone();
            next.status = "expired".to_string();
            let mut pipe = redis::pipe();
            pipe.hset(WHITELIST_RECORDS, id, serde_json::to_string(&next)?)
                .ignore();
            pipe.zrem(WHITELIST_RECORD_ORDER, id).ignore();
            pipe.zrem(WHITELIST_EXPIRY, id).ignore();
            queue_remove_whitelist_indexes(&mut pipe, &record);
            if self
                .execute_whitelist_record_pipeline_if_current(
                    &record,
                    TypedWhitelistMutation::Upsert(typed_whitelist_record(&next)?),
                    pipe,
                )
                .await?
            {
                return Ok(Some(record));
            }
        }
        Err(crate::storage::storage_error(format!(
            "whitelist record {id} kept changing during expiration"
        )))
    }

    pub async fn update_whitelist_comment(
        &self,
        id: &str,
        comment: String,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        for _ in 0..WHITELIST_MUTATION_MAX_RETRIES {
            let Some(previous) = self.get_whitelist_record(id).await? else {
                return Ok(None);
            };
            let mut next = previous.clone();
            next.comment = Some(comment.clone());
            let mut pipe = redis::pipe();
            pipe.hset(WHITELIST_RECORDS, id, serde_json::to_string(&next)?)
                .ignore();
            if self
                .execute_whitelist_record_pipeline_if_current(
                    &previous,
                    TypedWhitelistMutation::Upsert(typed_whitelist_record(&next)?),
                    pipe,
                )
                .await?
            {
                return Ok(Some(next));
            }
        }
        Err(crate::storage::storage_error(format!(
            "whitelist record {id} kept changing while updating its comment"
        )))
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
        let typed = self
            .typed_whitelist
            .load_one("region", id)
            .await
            .and_then(|document| document.map(whitelist_region_from_typed).transpose());
        let legacy = self.get_whitelist_region_group_legacy(id).await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => {
                self.observe_typed_whitelist_shadow_healthy();
                Ok(typed)
            }
            (Ok(_), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(
                    "typed whitelist region differs from legacy keyspace",
                );
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Err(error), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "typed whitelist region read failed; using legacy fallback: {error}"
                ));
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Ok(_), Err(error)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "legacy whitelist region comparison failed; refusing typed-only authorization data: {error}"
                ));
                Err(error)
            }
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy whitelist region reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn get_whitelist_region_group_legacy(
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
        drop(conn);
        let typed_regions = rewritten
            .iter()
            .map(typed_whitelist_region)
            .collect::<crate::storage::StorageResult<Vec<_>>>()?;
        self.execute_typed_whitelist_pipeline(
            TypedWhitelistMutation::ReplaceKind {
                kind: "region",
                documents: typed_regions,
            },
            pipe,
        )
        .await?;
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
        let typed = self
            .typed_whitelist
            .load_kind("region")
            .await
            .and_then(|documents| {
                let mut records = documents
                    .into_iter()
                    .map(whitelist_region_from_typed)
                    .collect::<crate::storage::StorageResult<Vec<_>>>()?;
                records.retain(WhitelistRegionGroupRecord::is_active);
                records.sort_by(|left, right| {
                    right
                        .created_at
                        .cmp(&left.created_at)
                        .then_with(|| right.id.cmp(&left.id))
                });
                Ok(records)
            });
        let legacy = self.list_whitelist_region_groups_legacy().await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => {
                self.observe_typed_whitelist_shadow_healthy();
                Ok(typed)
            }
            (Ok(_), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(
                    "typed whitelist region list differs from legacy keyspace",
                );
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Err(error), Ok(legacy)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "typed whitelist region list failed; using legacy fallback: {error}"
                ));
                self.rebuild_typed_whitelist_from_legacy().await?;
                Ok(legacy)
            }
            (Ok(_), Err(error)) => {
                self.observe_typed_whitelist_shadow_failure(&format!(
                    "legacy whitelist region comparison failed; refusing typed-only authorization data: {error}"
                ));
                Err(error)
            }
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy whitelist region list reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn list_whitelist_region_groups_legacy(
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
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_REGION_GROUP_RECORDS,
            &record.id,
            serde_json::to_string(record)?,
        )
        .ignore();
        pipe.zadd(WHITELIST_REGION_GROUP_ORDER, &record.id, record.created_at)
            .ignore();
        if let Some(expire_at) = record.expire_at {
            pipe.zadd(WHITELIST_REGION_GROUP_EXPIRY, &record.id, expire_at)
                .ignore();
        }
        self.execute_typed_whitelist_pipeline(
            TypedWhitelistMutation::Upsert(typed_whitelist_region(record)?),
            pipe,
        )
        .await
    }

    pub async fn delete_whitelist_region_group(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRegionGroupRecord>> {
        for _ in 0..WHITELIST_MUTATION_MAX_RETRIES {
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
            let mut pipe = redis::pipe();
            pipe.hset(
                WHITELIST_REGION_GROUP_RECORDS,
                id,
                serde_json::to_string(&next)?,
            )
            .ignore();
            pipe.zrem(WHITELIST_REGION_GROUP_ORDER, id).ignore();
            pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, id).ignore();
            if self
                .execute_whitelist_region_pipeline_if_current(
                    &record,
                    TypedWhitelistMutation::Upsert(typed_whitelist_region(&next)?),
                    pipe,
                )
                .await?
            {
                return Ok(Some(record));
            }
        }
        Err(crate::storage::storage_error(format!(
            "whitelist region group {id} kept changing during deletion"
        )))
    }

    pub async fn expire_whitelist_region_group(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRegionGroupRecord>> {
        for _ in 0..WHITELIST_MUTATION_MAX_RETRIES {
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
            let mut pipe = redis::pipe();
            pipe.hset(
                WHITELIST_REGION_GROUP_RECORDS,
                id,
                serde_json::to_string(&next)?,
            )
            .ignore();
            pipe.zrem(WHITELIST_REGION_GROUP_ORDER, id).ignore();
            pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, id).ignore();
            if self
                .execute_whitelist_region_pipeline_if_current(
                    &record,
                    TypedWhitelistMutation::Upsert(typed_whitelist_region(&next)?),
                    pipe,
                )
                .await?
            {
                return Ok(Some(record));
            }
        }
        Err(crate::storage::storage_error(format!(
            "whitelist region group {id} kept changing during expiration"
        )))
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

    async fn execute_whitelist_record_pipeline_if_current(
        &self,
        expected: &WhitelistRecord,
        mutation: TypedWhitelistMutation,
        pipe: redis::Pipeline,
    ) -> crate::storage::StorageResult<bool> {
        let expected_json = serde_json::to_string(expected)?;
        let expected_id = expected.id.clone();
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(
                    tokio_rusqlite::rusqlite::TransactionBehavior::Immediate,
                )?;
                let matched = redis::hash_field_matches_in_transaction(
                    &tx,
                    WHITELIST_RECORDS,
                    &expected_id,
                    |raw| {
                        raw.and_then(deserialize_whitelist_record)
                            .and_then(|record| serde_json::to_string(&record).ok())
                            .is_some_and(|current| current == expected_json)
                    },
                )?;
                if matched {
                    apply_typed_whitelist_mutation(&tx, mutation)?;
                    pipe.query_in_transaction::<()>(&tx)?;
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    async fn execute_whitelist_region_pipeline_if_current(
        &self,
        expected: &WhitelistRegionGroupRecord,
        mutation: TypedWhitelistMutation,
        pipe: redis::Pipeline,
    ) -> crate::storage::StorageResult<bool> {
        let expected_json = serde_json::to_string(expected)?;
        let expected_id = expected.id.clone();
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(
                    tokio_rusqlite::rusqlite::TransactionBehavior::Immediate,
                )?;
                let matched = redis::hash_field_matches_in_transaction(
                    &tx,
                    WHITELIST_REGION_GROUP_RECORDS,
                    &expected_id,
                    |raw| {
                        raw.and_then(deserialize_whitelist_region_group)
                            .and_then(|record| serde_json::to_string(&record).ok())
                            .is_some_and(|current| current == expected_json)
                    },
                )?;
                if matched {
                    apply_typed_whitelist_mutation(&tx, mutation)?;
                    pipe.query_in_transaction::<()>(&tx)?;
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    async fn execute_typed_whitelist_pipeline(
        &self,
        mutation: TypedWhitelistMutation,
        pipe: redis::Pipeline,
    ) -> crate::storage::StorageResult<()> {
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(
                    tokio_rusqlite::rusqlite::TransactionBehavior::Immediate,
                )?;
                apply_typed_whitelist_mutation(&tx, mutation)?;
                pipe.query_in_transaction::<()>(&tx)?;
                tx.commit()?;
                Ok(())
            })
            .await
    }
}

fn apply_typed_whitelist_mutation(
    tx: &tokio_rusqlite::rusqlite::Transaction<'_>,
    mutation: TypedWhitelistMutation,
) -> crate::storage::StorageResult<()> {
    match mutation {
        TypedWhitelistMutation::Upsert(document) => {
            TypedWhitelistRepository::upsert_tx(tx, &document)
        }
        TypedWhitelistMutation::Delete { kind, id } => {
            TypedWhitelistRepository::delete_tx(tx, kind, &id)
        }
        TypedWhitelistMutation::ReplaceKind { kind, documents } => {
            TypedWhitelistRepository::delete_kind_tx(tx, kind)?;
            for document in &documents {
                TypedWhitelistRepository::upsert_tx(tx, document)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod transaction_tests {
    use super::*;

    fn region_record() -> WhitelistRegionGroupRecord {
        WhitelistRegionGroupRecord {
            id: "whitelist-region:cas".to_string(),
            regions: vec![WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: None,
                operator: None,
            }],
            cidrs: vec!["192.0.2.0/24".to_string()],
            policy_id: String::new(),
            policy: None,
            source_cidr_count: 1,
            range_count: 1,
            expire_at: None,
            source: "manual".to_string(),
            created_at: 1,
            updated_at: 1,
            status: "active".to_string(),
            comment: None,
        }
    }

    #[tokio::test]
    async fn stale_region_cas_cannot_overwrite_compatibility_or_typed_state() {
        let directory = tempfile::tempdir().expect("create temp dir");
        let store = Store::connect(directory.path().join("fn-knock.sqlite3"))
            .await
            .expect("open store");
        let original = region_record();
        store
            .insert_whitelist_region_group(&original)
            .await
            .expect("insert original region");

        let mut fresh = original.clone();
        fresh.updated_at = 2;
        fresh.comment = Some("fresh".to_string());
        store
            .insert_whitelist_region_group(&fresh)
            .await
            .expect("write concurrent replacement");

        let mut stale_tombstone = original.clone();
        stale_tombstone.status = "deleted".to_string();
        stale_tombstone.cidrs.clear();
        stale_tombstone.source_cidr_count = 0;
        stale_tombstone.range_count = 0;
        let mut pipeline = redis::pipe();
        pipeline
            .hset(
                WHITELIST_REGION_GROUP_RECORDS,
                &original.id,
                serde_json::to_string(&stale_tombstone).unwrap(),
            )
            .ignore();
        pipeline
            .zrem(WHITELIST_REGION_GROUP_ORDER, &original.id)
            .ignore();

        let matched = store
            .execute_whitelist_region_pipeline_if_current(
                &original,
                TypedWhitelistMutation::Upsert(typed_whitelist_region(&stale_tombstone).unwrap()),
                pipeline,
            )
            .await
            .expect("run stale region CAS");
        assert!(!matched);

        let current = store
            .get_whitelist_region_group(&original.id)
            .await
            .unwrap()
            .expect("current region");
        assert_eq!(current.status, "active");
        assert_eq!(current.comment.as_deref(), Some("fresh"));
        let typed = store
            .typed_whitelist
            .load_one("region", &original.id)
            .await
            .unwrap()
            .expect("typed current region");
        assert_eq!(typed.document_json, serde_json::to_string(&fresh).unwrap());
    }
}
