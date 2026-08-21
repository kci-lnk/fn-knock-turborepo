use super::*;

impl Store {
    pub async fn get_config(&self) -> crate::storage::StorageResult<Value> {
        let (config, _) = self.load_typed_config_primary().await?;
        Ok(config)
    }

    pub(in crate::storage::redis_store) async fn reconcile_typed_config_from_legacy(
        &self,
    ) -> crate::storage::StorageResult<(Value, u64)> {
        self.load_typed_config_primary().await
    }

    pub(super) async fn load_typed_config_primary(
        &self,
    ) -> crate::storage::StorageResult<(Value, u64)> {
        let shadow = self
            .typed
            .typed_config
            .load_shadow(CONFIG_KEY, HOST_MAPPINGS_GENERATION_KEY)
            .await?;
        let legacy_snapshot =
            config_fence_snapshot_from_raw(shadow.legacy.config_raw, shadow.legacy.generation_raw)?;
        match shadow.typed {
            Ok(Some(typed))
                if typed.document == legacy_snapshot.config
                    && typed.host_mappings_generation == legacy_snapshot.generation =>
            {
                self.observe_typed_config_shadow(&legacy_snapshot, Ok(Some(typed.clone())));
                self.typed_config_primary_bootstrapped
                    .store(true, AtomicOrdering::Release);
                let mut config = typed.document;
                inject_config_generation_marker(&mut config, typed.host_mappings_generation)?;
                Ok((config, typed.revision))
            }
            typed => {
                // A newly-created database has no typed document until this
                // first read seeds it from the 2.x-compatible keyspace. That
                // expected bootstrap is not a shadow inconsistency. Once a
                // primary document has been established, missing data,
                // corruption, and content divergence are counted and
                // surfaced as fallbacks.
                let initial_bootstrap = matches!(&typed, Ok(None))
                    && !self
                        .typed_config_primary_bootstrapped
                        .load(AtomicOrdering::Acquire);
                if !initial_bootstrap {
                    self.observe_typed_config_shadow(&legacy_snapshot, typed);
                }
                let reconciled = self
                    .typed
                    .typed_config
                    .reconcile_from_legacy(
                        CONFIG_KEY,
                        HOST_MAPPINGS_GENERATION_KEY,
                        &default_config(),
                    )
                    .await?;
                let repaired_snapshot = config_fence_snapshot_from_raw(
                    reconciled.legacy.config_raw,
                    reconciled.legacy.generation_raw,
                )?;
                self.typed_config_shadow.set_healthy();
                self.typed_config_primary_bootstrapped
                    .store(true, AtomicOrdering::Release);
                tracing::info!(
                    typed_revision = reconciled.typed_revision,
                    host_mappings_generation = repaired_snapshot.generation,
                    "repaired typed config from legacy fallback"
                );
                let mut config = repaired_snapshot.config;
                inject_config_generation_marker(&mut config, repaired_snapshot.generation)?;
                Ok((config, reconciled.typed_revision))
            }
        }
    }

    pub(super) fn observe_typed_config_shadow(
        &self,
        snapshot: &ConfigFenceSnapshot,
        typed: crate::storage::StorageResult<
            Option<crate::storage::typed_config::TypedConfigDocument>,
        >,
    ) {
        match typed {
            Ok(Some(typed))
                if typed.document == snapshot.config
                    && typed.host_mappings_generation == snapshot.generation =>
            {
                if self.typed_config_shadow.mark_healthy() {
                    tracing::info!(
                        typed_revision = typed.revision,
                        host_mappings_generation = snapshot.generation,
                        "typed config shadow recovered"
                    );
                }
            }
            Ok(typed) => {
                if self.typed_config_shadow.mark_mismatch() {
                    tracing::warn!(
                        legacy_generation = snapshot.generation,
                        typed_generation = typed
                            .as_ref()
                            .map(|document| document.host_mappings_generation),
                        typed_revision = typed.as_ref().map(|document| document.revision),
                        typed_present = typed.is_some(),
                        "typed config mismatch; falling back to legacy keyspace and repairing typed primary"
                    );
                }
            }
            Err(error) => {
                if self.typed_config_shadow.mark_mismatch() {
                    tracing::warn!(
                        %error,
                        legacy_generation = snapshot.generation,
                        "typed config read failed; falling back to legacy keyspace and repairing typed primary"
                    );
                }
            }
        }
    }

    /// Atomically replaces the complete config only when the persisted
    /// generation and value still match `expected`. This is reserved for
    /// idempotent format migrations which must update host mappings and their
    /// shared policy table in one commit.
    pub async fn compare_and_set_config_migration(
        &self,
        expected: &Value,
        replacement: &Value,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut expected_config = expected.clone();
        take_config_generation_marker(&mut expected_config)?;
        strip_internal_config_metadata(&mut expected_config);
        let mut replacement_config = replacement.clone();
        strip_internal_config_metadata(&mut replacement_config);
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            if current_config != expected_config {
                return Ok(None);
            }
            let host_mappings_changed =
                config_host_mappings(&current_config) != config_host_mappings(&replacement_config);
            let replacement_generation = if host_mappings_changed {
                snapshot.generation.checked_add(1).ok_or_else(|| {
                    crate::storage::storage_error("host mappings generation overflow")
                })?
            } else {
                snapshot.generation
            };
            let replacement_raw = serde_json::to_string(&replacement_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                let mut published = replacement_config;
                inject_config_generation_marker(&mut published, replacement_generation)?;
                self.publish_config_snapshot(published.clone(), revision);
                return Ok(Some(published));
            }
        }
        Ok(None)
    }

    /// Atomically replaces only the `host_mappings` section when its current
    /// value still exactly matches `expected`.
    ///
    /// The returned value is the complete config that was persisted. This is
    /// intentionally produced inside the storage transaction so callers do
    /// not have to reconstruct a full config from a stale read and therefore
    /// cannot overwrite unrelated top-level sections.
    pub async fn compare_and_set_host_mappings(
        &self,
        expected: &[Value],
        replacement: &[Value],
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.compare_and_set_host_mappings_inner(expected, replacement, None)
            .await
    }

    pub async fn compare_and_set_host_mappings_with_visibility_policies(
        &self,
        expected: &[Value],
        replacement: &[Value],
        visibility_policies: &Map<String, Value>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.compare_and_set_host_mappings_inner(expected, replacement, Some(visibility_policies))
            .await
    }

    pub(super) async fn compare_and_set_host_mappings_inner(
        &self,
        expected: &[Value],
        replacement: &[Value],
        visibility_policies: Option<&Map<String, Value>>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        // An unrelated top-level section may change between our read and the
        // raw-string CAS. Re-read and merge in that case; if host_mappings
        // itself changed, the exact structural comparison below returns a
        // conflict instead of overwriting the newer value.
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(current_object) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let current_mappings = match current_object.get("host_mappings") {
                None => &[][..],
                Some(Value::Array(mappings)) => mappings.as_slice(),
                Some(_) => return Ok(None),
            };
            if current_mappings != expected {
                return Ok(None);
            }
            let mappings_changed = current_mappings != replacement;
            current_object.insert(
                "host_mappings".to_string(),
                Value::Array(replacement.to_vec()),
            );
            if let Some(visibility_policies) = visibility_policies {
                replace_visibility_policies_for_host_mappings(
                    &mut current_config,
                    replacement,
                    visibility_policies,
                )?;
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            let replacement_generation = if mappings_changed {
                snapshot.generation.checked_add(1).ok_or_else(|| {
                    crate::storage::storage_error("host mappings generation overflow")
                })?
            } else {
                snapshot.generation
            };
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, replacement_generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(Some(current_config));
            }
        }
        Ok(None)
    }

    /// Atomically replaces the Host mapping list and its UI grouping catalog.
    /// The shared generation advances when either section changes so a stale
    /// full-config writer cannot overwrite a concurrent organization update.
    #[cfg(test)]
    pub async fn compare_and_set_host_mapping_catalog(
        &self,
        expected_mappings: &[Value],
        expected_groups: &[Value],
        expected_grouped_view: bool,
        replacement_mappings: &[Value],
        replacement_groups: &[Value],
        replacement_grouped_view: bool,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.compare_and_set_host_mapping_catalog_inner(
            expected_mappings,
            expected_groups,
            expected_grouped_view,
            replacement_mappings,
            replacement_groups,
            replacement_grouped_view,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn compare_and_set_host_mapping_catalog_with_visibility_policies(
        &self,
        expected_mappings: &[Value],
        expected_groups: &[Value],
        expected_grouped_view: bool,
        replacement_mappings: &[Value],
        replacement_groups: &[Value],
        replacement_grouped_view: bool,
        visibility_policies: &Map<String, Value>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.compare_and_set_host_mapping_catalog_inner(
            expected_mappings,
            expected_groups,
            expected_grouped_view,
            replacement_mappings,
            replacement_groups,
            replacement_grouped_view,
            Some(visibility_policies),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn compare_and_set_host_mapping_catalog_inner(
        &self,
        expected_mappings: &[Value],
        expected_groups: &[Value],
        expected_grouped_view: bool,
        replacement_mappings: &[Value],
        replacement_groups: &[Value],
        replacement_grouped_view: bool,
        visibility_policies: Option<&Map<String, Value>>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(current_object) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let current_mappings = match current_object.get("host_mappings") {
                None => &[][..],
                Some(Value::Array(items)) => items.as_slice(),
                Some(_) => return Ok(None),
            };
            let current_groups = match current_object.get("host_mapping_groups") {
                None => &[][..],
                Some(Value::Array(items)) => items.as_slice(),
                Some(_) => return Ok(None),
            };
            let current_grouped_view = current_object
                .get("host_mapping_grouped_view")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if current_mappings != expected_mappings
                || current_groups != expected_groups
                || current_grouped_view != expected_grouped_view
            {
                return Ok(None);
            }

            let changed = current_mappings != replacement_mappings
                || current_groups != replacement_groups
                || current_grouped_view != replacement_grouped_view;
            current_object.insert(
                "host_mappings".to_string(),
                Value::Array(replacement_mappings.to_vec()),
            );
            current_object.insert(
                "host_mapping_groups".to_string(),
                Value::Array(replacement_groups.to_vec()),
            );
            current_object.insert(
                "host_mapping_grouped_view".to_string(),
                Value::Bool(replacement_grouped_view),
            );
            if let Some(visibility_policies) = visibility_policies {
                replace_visibility_policies_for_host_mappings(
                    &mut current_config,
                    replacement_mappings,
                    visibility_policies,
                )?;
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            let replacement_generation = if changed {
                snapshot.generation.checked_add(1).ok_or_else(|| {
                    crate::storage::storage_error("host mapping catalog generation overflow")
                })?
            } else {
                snapshot.generation
            };
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, replacement_generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(Some(current_config));
            }
        }
        Ok(None)
    }

    /// Atomically merges the two gateway target configuration sections into
    /// the latest full config. Host runtime synchronization may overlap both
    /// a non-Host writer (for example a run_type update) and a newer writer of
    /// either target section. A section is replaced only while its exact
    /// original value, including absence, still matches; otherwise the newer
    /// stored section is retained.
    pub async fn merge_gateway_target_config_sections(
        &self,
        expected_gateway_proxy_headers: Option<&Value>,
        gateway_proxy_headers: &Value,
        expected_gateway_host_response: Option<&Value>,
        gateway_host_response: &Value,
    ) -> crate::storage::StorageResult<Value> {
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(object) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let proxy_headers_unchanged = match (
                object.get("gateway_proxy_headers"),
                expected_gateway_proxy_headers,
            ) {
                (None, None) => true,
                (Some(current), Some(expected)) => current == expected,
                _ => false,
            };
            if proxy_headers_unchanged {
                object.insert(
                    "gateway_proxy_headers".to_string(),
                    gateway_proxy_headers.clone(),
                );
            }
            let host_response_unchanged = match (
                object.get("gateway_host_response"),
                expected_gateway_host_response,
            ) {
                (None, None) => true,
                (Some(current), Some(expected)) => current == expected,
                _ => false,
            };
            if host_response_unchanged {
                object.insert(
                    "gateway_host_response".to_string(),
                    gateway_host_response.clone(),
                );
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(current_config);
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while merging gateway target sections",
        ))
    }

    /// Atomically replaces one ordinary top-level config value while retaining
    /// every unrelated field from the latest stored snapshot. Host mapping
    /// catalog fields have dedicated generation-aware APIs and must not use
    /// this helper.
    pub async fn set_config_top_level_value(
        &self,
        key: &str,
        value: Value,
    ) -> crate::storage::StorageResult<Value> {
        if matches!(
            key,
            "host_mappings"
                | "host_mapping_groups"
                | "host_mapping_grouped_view"
                | "visibility_policies"
        ) {
            return Err(crate::storage::storage_error(
                "host mapping catalog fields require a generation-aware config API",
            ));
        }

        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(object) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            object.insert(key.to_string(), value.clone());

            let replacement_raw = serde_json::to_string(&current_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(current_config);
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while setting a top-level value",
        ))
    }

    /// Atomically merges fields into an object-valued top-level config
    /// section. Each CAS retry starts from the latest stored config, so
    /// independent writers cannot replace one another with stale snapshots.
    pub async fn merge_config_object_fields(
        &self,
        section: &str,
        fields: Map<String, Value>,
    ) -> crate::storage::StorageResult<Value> {
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(root) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let section_value = root
                .entry(section.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !section_value.is_object() {
                *section_value = Value::Object(Map::new());
            }
            let section_object = section_value
                .as_object_mut()
                .expect("object value was initialized above");
            section_object.extend(fields.clone());

            let replacement_raw = serde_json::to_string(&current_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(current_config);
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while merging object fields",
        ))
    }

    /// Atomically replaces the SSL section only while it still exactly
    /// matches the caller's expected value. Unrelated top-level configuration
    /// writes are merged from the latest snapshot, while a concurrent SSL
    /// writer produces a conflict instead of being overwritten.
    pub async fn compare_and_set_ssl_config(
        &self,
        expected: Option<&Value>,
        replacement: Option<&Value>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(object) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let ssl_unchanged = match (object.get("ssl"), expected) {
                (None, None) => true,
                (Some(current), Some(expected)) => current == expected,
                _ => false,
            };
            if !ssl_unchanged {
                return Ok(None);
            }
            match replacement {
                Some(replacement) => {
                    object.insert("ssl".to_string(), replacement.clone());
                }
                None => {
                    object.remove("ssl");
                }
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(Some(current_config));
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while replacing SSL configuration",
        ))
    }

    pub async fn save_config(&self, value: &Value) -> crate::storage::StorageResult<()> {
        let mut requested_config = value.clone();
        let requested_generation = take_config_generation_marker(&mut requested_config)?;
        strip_internal_config_metadata(&mut requested_config);
        let requested_host_mappings = config_host_mappings(&requested_config);
        let requested_host_fingerprint = config_host_mappings_fingerprint(&requested_config)?;
        if let Some(marker) = requested_generation.as_ref()
            && marker.host_fingerprint != requested_host_fingerprint
        {
            return Err(crate::storage::storage_error(
                "host mappings must be updated through compare_and_set_host_mappings",
            ));
        }
        let mut conn = self.conn();

        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let current_host_mappings = config_host_mappings(&snapshot.config);
            let current_host_fingerprint = config_host_mappings_fingerprint(&snapshot.config)?;
            let mut replacement_config = requested_config.clone();
            let replacement_generation = match requested_generation.as_ref() {
                Some(marker)
                    if marker.host_fingerprint != current_host_fingerprint
                        || marker.generation != snapshot.generation =>
                {
                    return Err(crate::storage::storage_error(
                        "host mappings changed after this config snapshot was read",
                    ));
                }
                Some(_) => snapshot.generation,
                None => {
                    if snapshot.config_raw.is_some() {
                        return Err(crate::storage::storage_error(
                            "config generation marker is required for an existing config",
                        ));
                    }
                    if requested_host_mappings == current_host_mappings {
                        snapshot.generation
                    } else {
                        snapshot.generation.checked_add(1).ok_or_else(|| {
                            crate::storage::storage_error("host mappings generation overflow")
                        })?
                    }
                }
            };
            strip_internal_config_metadata(&mut replacement_config);
            let replacement_raw = serde_json::to_string(&replacement_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut replacement_config, replacement_generation)?;
                self.publish_config_snapshot(replacement_config, revision);
                return Ok(());
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while saving",
        ))
    }

    /// Explicitly replaces the complete persisted config. Normal application
    /// updates must use `get_config` followed by `save_config`; this test-only
    /// method sets up explicit full replacements.
    #[cfg(test)]
    pub async fn replace_config(&self, value: &Value) -> crate::storage::StorageResult<()> {
        let mut replacement_config = value.clone();
        strip_internal_config_metadata(&mut replacement_config);
        let replacement_host_mappings = config_host_mappings(&replacement_config);
        let replacement_raw = serde_json::to_string(&replacement_config)?;
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let replacement_generation =
                if replacement_host_mappings == config_host_mappings(&snapshot.config) {
                    snapshot.generation
                } else {
                    snapshot.generation.checked_add(1).ok_or_else(|| {
                        crate::storage::storage_error("host mappings generation overflow")
                    })?
                };
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                let mut published_config = replacement_config.clone();
                inject_config_generation_marker(&mut published_config, replacement_generation)?;
                self.publish_config_snapshot(published_config, revision);
                return Ok(());
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while replacing",
        ))
    }

    pub async fn locale(&self) -> crate::storage::StorageResult<Value> {
        let config = self.get_config().await?;
        Ok(config
            .get("locale")
            .cloned()
            .unwrap_or_else(|| json!({ "default_locale": "zh-CN" })))
    }

    pub async fn appearance(&self) -> crate::storage::StorageResult<Value> {
        let config = self.get_config().await?;
        Ok(config
            .get("appearance")
            .cloned()
            .unwrap_or_else(|| json!({ "theme_color_preset": "default" })))
    }
}
