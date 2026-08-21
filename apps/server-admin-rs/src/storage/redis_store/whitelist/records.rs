use super::*;

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

    pub(super) fn observe_typed_whitelist_shadow_healthy(&self) {
        if self.typed_whitelist_shadow.mark_healthy() {
            tracing::info!("typed whitelist document comparison recovered");
        }
    }

    pub(super) fn observe_typed_whitelist_shadow_failure(&self, reason: &str) {
        if self.typed_whitelist_shadow.mark_mismatch() {
            tracing::warn!(%reason, "typed whitelist document comparison failed");
        }
    }

    #[cfg(test)]
    pub(crate) fn typed_whitelist_shadow_mismatch_count(&self) -> u64 {
        self.typed_whitelist_shadow.mismatch_count()
    }

    pub async fn get_whitelist_record(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRecord>> {
        let typed = self
            .typed
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

    pub(super) async fn get_whitelist_record_legacy(
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
            .typed
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

    pub(super) async fn list_whitelist_records_legacy(
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

    pub(super) async fn try_replace_whitelist_record(
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
            .typed
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

    pub(super) async fn get_whitelist_region_group_legacy(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<WhitelistRegionGroupRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.hget(WHITELIST_REGION_GROUP_RECORDS, id).await?;
        Ok(raw.and_then(|value| deserialize_whitelist_region_group(&value)))
    }
}
