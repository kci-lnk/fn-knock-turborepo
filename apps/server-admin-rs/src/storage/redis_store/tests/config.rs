use super::*;

#[tokio::test]
async fn presentation_config_reads_use_snapshot_while_primary_is_busy() {
    let (_dir, store) = open_test_store().await;
    let mut config = (*store.config_snapshot()).clone();
    config["locale"] = json!({ "default_locale": "en" });
    config["appearance"] = json!({ "theme_color_preset": "forest" });
    store.save_config(&config).await.unwrap();

    let (release, blocker) = block_primary_executor(&store).await;
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::try_join!(store.locale(), store.appearance())
    })
    .await;
    release.send(()).unwrap();
    blocker.await.unwrap();
    let (locale, appearance) = result
        .expect("presentation reads must not queue for SQLite")
        .unwrap();
    assert_eq!(locale, config["locale"]);
    assert_eq!(appearance, config["appearance"]);
}

#[tokio::test]
async fn presentation_config_snapshot_refreshes_after_another_store_writes() {
    let (_dir, reader) = open_test_store().await;
    let writer = Store::connect(&reader.path).await.unwrap();
    let before = reader.config_snapshot();
    let mut updated = writer.get_config().await.unwrap();
    updated["locale"] = json!({ "default_locale": "ja-JP" });
    updated["appearance"] = json!({ "theme_color_preset": "forest" });
    writer.save_config(&updated).await.unwrap();

    let observed = reader.get_config().await.unwrap();
    assert_eq!(observed["locale"], updated["locale"]);
    assert_eq!(reader.locale().await.unwrap(), updated["locale"]);
    assert_eq!(reader.appearance().await.unwrap(), updated["appearance"]);
    let refreshed = reader.config_snapshot();
    assert!(!Arc::ptr_eq(&before, &refreshed));

    reader.get_config().await.unwrap();
    assert!(
        Arc::ptr_eq(&refreshed, &reader.config_snapshot()),
        "an unchanged revision must keep the existing snapshot allocation"
    );
}

#[tokio::test]
async fn presentation_config_snapshot_tracks_mutations_migrations_and_restore() {
    let (_dir, store) = open_test_store().await;
    store
        .set_config_top_level_value("locale", json!({ "default_locale": "ja-JP" }))
        .await
        .unwrap();
    store
        .merge_config_object_fields(
            "appearance",
            Map::from_iter([("theme_color_preset".to_string(), json!("forest"))]),
        )
        .await
        .unwrap();
    assert_eq!(store.locale().await.unwrap()["default_locale"], "ja-JP");
    assert_eq!(
        store.appearance().await.unwrap()["theme_color_preset"],
        "forest"
    );

    let expected = store.get_config().await.unwrap();
    let mut migrated = expected.clone();
    migrated["locale"] = json!({ "default_locale": "ko-KR" });
    migrated["appearance"] = json!({ "theme_color_preset": "ocean" });
    store
        .compare_and_set_config_migration(&expected, &migrated)
        .await
        .unwrap()
        .expect("migration CAS");
    assert_eq!(store.locale().await.unwrap(), migrated["locale"]);
    assert_eq!(store.appearance().await.unwrap(), migrated["appearance"]);

    let restored = json!({
        "locale": { "default_locale": "en" },
        "appearance": { "theme_color_preset": "sunset" },
        "host_mappings": []
    });
    store
        .replace_backup_entries_by_prefix(
            "fn_knock:",
            &[json!({
                "key": CONFIG_KEY,
                "type": "string",
                "ttl_ms": null,
                "value": restored.to_string()
            })],
            200,
        )
        .await
        .unwrap();
    assert_eq!(store.locale().await.unwrap(), restored["locale"]);
    assert_eq!(store.appearance().await.unwrap(), restored["appearance"]);

    let reopened = Store::connect(&store.path).await.unwrap();
    assert_eq!(reopened.locale().await.unwrap(), restored["locale"]);
    assert_eq!(reopened.appearance().await.unwrap(), restored["appearance"]);
    drop(reopened);

    store.clear_all_keys().await.unwrap();
    let defaults = default_config();
    assert_eq!(store.locale().await.unwrap(), defaults["locale"]);
    assert_eq!(store.appearance().await.unwrap(), defaults["appearance"]);
}

#[tokio::test]
async fn presentation_config_snapshot_tracks_compatibility_writes_and_deletes() {
    let (_dir, store) = open_test_store().await;
    let config = json!({ "locale": { "default_locale": "en" } });
    store.set_json_value(CONFIG_KEY, &config).await.unwrap();
    assert_eq!(store.locale().await.unwrap(), config["locale"]);

    let mut updated = config;
    updated["locale"] = json!({ "default_locale": "ja-JP" });
    store
        .set_string_value(CONFIG_KEY, &updated.to_string())
        .await
        .unwrap();
    assert_eq!(store.locale().await.unwrap(), updated["locale"]);

    updated["locale"] = json!({ "default_locale": "ko-KR" });
    store
        .set_json_values_atomically(&[(CONFIG_KEY, &updated)])
        .await
        .unwrap();
    assert_eq!(store.locale().await.unwrap(), updated["locale"]);
    store.delete_key(CONFIG_KEY).await.unwrap();
    assert_eq!(store.locale().await.unwrap(), default_config()["locale"]);
    store.set_json_value(CONFIG_KEY, &updated).await.unwrap();
    store.delete_keys(&[CONFIG_KEY.to_string()]).await.unwrap();
    assert_eq!(store.locale().await.unwrap(), default_config()["locale"]);
}

#[tokio::test]
async fn poll_log_buffer_recovers_when_sequence_lags_existing_items() {
    let (_dir, store) = open_test_store().await;
    let key = "fn_knock:test:logs";
    let seq_key = format!("{key}:seq");
    let mut conn = store.conn();
    let _: () = redis::cmd("RPUSH")
        .arg(key)
        .arg("old")
        .arg("middle")
        .arg("latest")
        .query_async(&mut conn)
        .await
        .expect("seed log list");
    conn.set(&seq_key, 2).await.expect("seed stale seq");

    let result = store
        .poll_log_buffer(key, Some("2"))
        .await
        .expect("poll logs");

    assert_eq!(result["cursor"], json!(3));
    assert_eq!(result["reset"], json!(false));
    assert_eq!(result["items"], json!(["latest"]));
}

#[tokio::test]
async fn append_log_buffer_continues_sequence_from_existing_items_without_seq() {
    let (_dir, store) = open_test_store().await;
    let key = "fn_knock:test:logs-without-seq";
    let mut conn = store.conn();
    let _: () = redis::cmd("RPUSH")
        .arg(key)
        .arg("old")
        .query_async(&mut conn)
        .await
        .expect("seed log list without seq");

    store
        .append_log_buffer(key, &["new-1".to_string(), "new-2".to_string()], 60, 100)
        .await
        .expect("append logs");

    let result = store
        .poll_log_buffer(key, Some("1"))
        .await
        .expect("poll appended logs");
    assert_eq!(result["cursor"], json!(3));
    assert_eq!(result["reset"], json!(false));
    assert_eq!(result["items"], json!(["new-1", "new-2"]));

    let empty = store
        .poll_log_buffer(key, Some("3"))
        .await
        .expect("poll after current cursor");
    assert_eq!(empty["cursor"], json!(3));
    assert_eq!(empty["items"], json!([]));
}

#[tokio::test]
async fn json_locks_refresh_and_release_only_for_the_owner() {
    let (_dir, store) = open_test_store().await;
    let key = "fn_knock:test:json-lock";
    let initial = json!({ "lockId": "owner", "createdAt": "initial" });
    assert!(
        store
            .set_json_value_nx_ex(key, &initial, 30)
            .await
            .expect("acquire JSON lock")
    );

    assert!(
        !store
            .set_json_lock_if_owned_ex(
                key,
                "other",
                &json!({ "lockId": "other", "createdAt": "wrong" }),
                120,
            )
            .await
            .expect("reject wrong owner refresh")
    );
    assert_eq!(
        store.get_json_value(key).await.unwrap(),
        Some(initial.clone())
    );

    let refreshed = json!({ "lockId": "owner", "createdAt": "refreshed" });
    assert!(
        store
            .set_json_lock_if_owned_ex(key, "owner", &refreshed, 120)
            .await
            .expect("refresh owned lock")
    );
    assert_eq!(store.get_json_value(key).await.unwrap(), Some(refreshed));
    let mut conn = store.conn();
    assert!(conn.ttl(key).await.expect("read refreshed TTL") > 30);

    assert!(
        !store
            .delete_lock_if_owned(key, "other")
            .await
            .expect("reject wrong owner release")
    );
    assert!(store.get_json_value(key).await.unwrap().is_some());
    assert!(
        store
            .delete_lock_if_owned(key, "owner")
            .await
            .expect("release owned lock")
    );
    assert_eq!(store.get_json_value(key).await.unwrap(), None);
    assert!(
        !store
            .delete_lock_if_owned(key, "owner")
            .await
            .expect("repeat release")
    );

    store
        .set_string_value(key, "not-json")
        .await
        .expect("seed invalid JSON lock");
    assert!(
        !store
            .delete_lock_if_owned(key, "owner")
            .await
            .expect("invalid JSON is not owned")
    );
    assert_eq!(
        store.get_string_value(key).await.unwrap().as_deref(),
        Some("not-json")
    );
}

#[tokio::test]
async fn host_mapping_section_cas_requires_an_exact_array_and_preserves_other_sections() {
    let (_dir, store) = open_test_store().await;
    let structurally_invalid = json!({
        "host_mappings": {},
        "unrelated": { "generation": 7 }
    });
    store
        .save_config(&structurally_invalid)
        .await
        .expect("seed non-array host mappings");

    assert!(
        store
            .compare_and_set_host_mappings(&[], &[json!({ "host": "a.example.com" })])
            .await
            .expect("compare structural mismatch")
            .is_none()
    );
    let mut stored = store.get_config().await.unwrap();
    strip_internal_config_metadata(&mut stored);
    assert_eq!(stored, structurally_invalid);

    let expected = vec![json!({
        "host": "a.example.com",
        "target": "http://127.0.0.1:8080"
    })];
    store
        .replace_config(&json!({
            "host_mappings": expected,
            "unrelated": { "generation": 8 }
        }))
        .await
        .expect("seed valid host mappings");
    let expected = store.get_config().await.unwrap()["host_mappings"]
        .as_array()
        .cloned()
        .unwrap();
    let mut forbidden_full_writer = store.get_config().await.unwrap();
    forbidden_full_writer["host_mappings"] = json!([{
        "host": "forbidden.example.com"
    }]);
    assert!(store.save_config(&forbidden_full_writer).await.is_err());
    assert_eq!(
        store.get_config().await.unwrap()["host_mappings"],
        json!(expected)
    );
    let replacement = vec![json!({
        "host": "a.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1"
    })];
    let updated = store
        .compare_and_set_host_mappings(&expected, &replacement)
        .await
        .expect("apply exact host mappings CAS")
        .expect("expected value matched");

    assert_eq!(updated["host_mappings"], json!(replacement));
    assert_eq!(updated["unrelated"]["generation"], json!(8));
    assert_eq!(store.get_config().await.unwrap(), updated);

    let emptied = store
        .compare_and_set_host_mappings(&replacement, &[])
        .await
        .expect("replace host mappings with an empty array")
        .expect("non-empty expected value matched");
    assert_eq!(emptied["host_mappings"], json!([]));
    assert!(emptied["host_mappings"].is_array());
    assert_eq!(emptied["unrelated"]["generation"], json!(8));
    assert_eq!(store.get_config().await.unwrap(), emptied);
}

#[tokio::test]
async fn host_mapping_policy_cas_keeps_advanced_auth_references_and_restores_old_policies() {
    let (_dir, store) = open_test_store().await;
    let old_mappings = vec![json!({
        "host": "app.example.com",
        "visibility": { "policy_id": "visibility-old" },
        "advanced_auth": {
            "groups": [{
                "conditions": [{ "policy_id": "advanced-old" }]
            }]
        }
    })];
    let old_policies = json!({
        "visibility-old": { "format_version": 2 },
        "advanced-old": { "format_version": 2 },
        "unreferenced": { "format_version": 2 }
    });
    store
        .save_config(&json!({
            "host_mappings": old_mappings,
            "visibility_policies": old_policies,
        }))
        .await
        .expect("seed config");

    let new_mappings = vec![json!({
        "host": "app.example.com",
        "visibility": { "policy_id": "visibility-old" },
        "advanced_auth": {
            "groups": [{
                "conditions": [{ "policy_id": "advanced-new" }]
            }]
        }
    })];
    let supplied = json!({
        "visibility-old": { "format_version": 2 },
        "advanced-new": { "format_version": 2 }
    })
    .as_object()
    .cloned()
    .unwrap();
    let updated = store
        .compare_and_set_host_mappings_with_visibility_policies(
            &old_mappings,
            &new_mappings,
            &supplied,
        )
        .await
        .expect("update mappings and policies")
        .expect("old mappings matched");
    assert!(
        updated["visibility_policies"]
            .get("visibility-old")
            .is_some()
    );
    assert!(updated["visibility_policies"].get("advanced-new").is_some());
    assert!(updated["visibility_policies"].get("advanced-old").is_none());
    assert!(updated["visibility_policies"].get("unreferenced").is_none());

    let old_policies = old_policies.as_object().cloned().unwrap();
    let restored = store
        .compare_and_set_host_mappings_with_visibility_policies(
            &new_mappings,
            &old_mappings,
            &old_policies,
        )
        .await
        .expect("rollback mappings and policies")
        .expect("new mappings matched");
    assert!(
        restored["visibility_policies"]
            .get("visibility-old")
            .is_some()
    );
    assert!(
        restored["visibility_policies"]
            .get("advanced-old")
            .is_some()
    );
    assert!(
        restored["visibility_policies"]
            .get("advanced-new")
            .is_none()
    );
}

#[tokio::test]
async fn config_snapshot_is_published_immediately_after_save_and_host_cas() {
    let (_dir, store) = open_test_store().await;
    let before = store.config_snapshot();
    assert!(before.get("snapshot_test").is_none());

    let mut saved = (*before).clone();
    saved["snapshot_test"] = json!({"generation": 1});
    store.save_config(&saved).await.expect("save config");
    let after_save = store.config_snapshot();
    assert_eq!(
        after_save.pointer("/snapshot_test/generation"),
        Some(&json!(1))
    );
    assert!(!Arc::ptr_eq(&before, &store.config_snapshot()));

    let replacement = vec![json!({
        "host": "snapshot.example.com",
        "target": "http://127.0.0.1:8080"
    })];
    store
        .compare_and_set_host_mappings(&[], &replacement)
        .await
        .expect("host CAS")
        .expect("host CAS matched");
    let after_cas = store.config_snapshot();
    assert_eq!(after_cas["host_mappings"], json!(replacement));
    assert_eq!(
        after_cas.pointer("/snapshot_test/generation"),
        Some(&json!(1))
    );
}

#[tokio::test]
async fn host_mapping_catalog_cas_updates_both_sections_atomically() {
    let (_dir, store) = open_test_store().await;
    let mappings = vec![json!({
        "host": "app.example.com",
        "target": "http://127.0.0.1:8080",
        "group_id": null
    })];
    store
        .save_config(&json!({
            "host_mappings": mappings,
            "host_mapping_groups": [],
            "unrelated": { "preserved": true }
        }))
        .await
        .expect("seed config");
    let groups = vec![json!({
        "id": "11111111-1111-4111-8111-111111111111",
        "name": "Media"
    })];
    let grouped = vec![json!({
        "host": "app.example.com",
        "target": "http://127.0.0.1:8080",
        "group_id": "11111111-1111-4111-8111-111111111111"
    })];
    let updated = store
        .compare_and_set_host_mapping_catalog(&mappings, &[], false, &grouped, &groups, true)
        .await
        .expect("catalog CAS")
        .expect("catalog matched");
    assert_eq!(updated["host_mappings"], json!(grouped));
    assert_eq!(updated["host_mapping_groups"], json!(groups));
    assert_eq!(updated["host_mapping_grouped_view"], json!(true));
    assert_eq!(updated["unrelated"]["preserved"], json!(true));

    assert!(
        store
            .compare_and_set_host_mapping_catalog(&mappings, &[], false, &[], &[], false)
            .await
            .expect("stale catalog CAS")
            .is_none()
    );
}

#[tokio::test]
async fn gateway_target_section_merge_preserves_newer_config_and_section_writes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let stale_store = Store::connect(&path).await.expect("open stale store");
    let newer_store = Store::connect(&path).await.expect("open newer store");
    stale_store
        .save_config(&json!({
            "run_type": 3,
            "host_mappings": [{
                "host": "video.example.com",
                "target": "http://127.0.0.1:8080",
                "protocol_mode": "http1"
            }],
            "gateway_proxy_headers": { "disabled_hosts": [] },
            "gateway_host_response": { "disabled_hosts": [] },
            "unrelated": { "generation": 1 }
        }))
        .await
        .expect("seed config");

    let stale = stale_store.get_config().await.expect("load stale config");
    let expected_proxy_headers = stale.get("gateway_proxy_headers").cloned();
    let expected_host_response = stale.get("gateway_host_response").cloned();

    let mut newer = newer_store.get_config().await.expect("load newer config");
    newer["run_type"] = json!(0);
    newer["gateway_proxy_headers"] = json!({
        "disabled_hosts": ["video.example.com"]
    });
    newer["unrelated"]["generation"] = json!(2);
    newer_store
        .save_config(&newer)
        .await
        .expect("save interleaved config update");

    let merged = stale_store
        .merge_gateway_target_config_sections(
            expected_proxy_headers.as_ref(),
            &json!({ "disabled_hosts": [] }),
            expected_host_response.as_ref(),
            &json!({ "disabled_hosts": ["video.example.com"] }),
        )
        .await
        .expect("merge stale target sections");

    assert_eq!(merged["run_type"], json!(0));
    assert_eq!(merged["unrelated"]["generation"], json!(2));
    assert_eq!(
        merged["gateway_proxy_headers"],
        json!({ "disabled_hosts": ["video.example.com"] }),
        "a newer write to the same section must win"
    );
    assert_eq!(
        merged["gateway_host_response"],
        json!({ "disabled_hosts": ["video.example.com"] }),
        "an unchanged section may receive the compiled replacement"
    );
    assert_eq!(stale_store.get_config().await.unwrap(), merged);
}

#[tokio::test]
async fn json_rewrite_preserves_ttl_without_resurrecting_expired_keys() {
    let (_dir, store) = open_test_store().await;
    let key = "fn_knock:test:preserve-ttl";
    store
        .set_json_value_ex(key, &json!({ "version": 1 }), 120)
        .await
        .unwrap();
    let (_, ttl) = store.get_json_value_with_ttl(key).await.unwrap();
    store
        .set_json_value_preserve_ttl(key, &json!({ "version": 2 }), ttl)
        .await
        .unwrap();
    let (_, rewritten_ttl) = store.get_json_value_with_ttl(key).await.unwrap();
    assert!(rewritten_ttl > 0 && rewritten_ttl <= ttl);

    store
        .set_json_value_preserve_ttl(key, &json!({ "version": 3 }), -2)
        .await
        .unwrap();
    assert!(store.get_json_value(key).await.unwrap().is_none());
}

#[tokio::test]
async fn object_field_merge_preserves_concurrent_scan_discovery_writes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let target_store = Store::connect(&path).await.expect("open target store");
    let intensity_store = Store::connect(&path).await.expect("open intensity store");
    target_store
        .save_config(&json!({
            "host_mappings": [],
            "scan_discovery": {
                "custom_cidrs": [],
                "selected_cidrs": [],
                "intensity_mode": "auto",
                "intensity_level": "medium"
            }
        }))
        .await
        .expect("seed scan discovery config");

    let targets = target_store.merge_config_object_fields(
        "scan_discovery",
        [
            ("custom_cidrs".to_string(), json!(["10.0.0.0/24"])),
            ("selected_cidrs".to_string(), json!(["192.168.1.0/24"])),
        ]
        .into_iter()
        .collect(),
    );
    let intensity = intensity_store.merge_config_object_fields(
        "scan_discovery",
        [
            ("intensity_mode".to_string(), json!("manual")),
            ("intensity_level".to_string(), json!("high")),
        ]
        .into_iter()
        .collect(),
    );
    let (targets_result, intensity_result) = tokio::join!(targets, intensity);
    targets_result.expect("merge target fields");
    intensity_result.expect("merge intensity fields");

    let merged = target_store.get_config().await.expect("load merged config");
    assert_eq!(
        merged["scan_discovery"]["custom_cidrs"],
        json!(["10.0.0.0/24"])
    );
    assert_eq!(
        merged["scan_discovery"]["selected_cidrs"],
        json!(["192.168.1.0/24"])
    );
    assert_eq!(merged["scan_discovery"]["intensity_mode"], "manual");
    assert_eq!(merged["scan_discovery"]["intensity_level"], "high");
}

#[tokio::test]
async fn gateway_target_section_merge_distinguishes_absent_from_present() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let stale_store = Store::connect(&path).await.expect("open stale store");
    let newer_store = Store::connect(&path).await.expect("open newer store");
    stale_store
        .save_config(&json!({
            "run_type": 3,
            "host_mappings": []
        }))
        .await
        .expect("seed config without target sections");

    let mut newer = newer_store.get_config().await.expect("load newer config");
    newer["gateway_proxy_headers"] = json!({ "disabled_hosts": ["new.example.com"] });
    newer_store
        .save_config(&newer)
        .await
        .expect("add proxy section concurrently");

    let merged = stale_store
        .merge_gateway_target_config_sections(
            None,
            &json!({ "disabled_hosts": [] }),
            None,
            &json!({ "disabled_hosts": ["compiled.example.com"] }),
        )
        .await
        .expect("merge sections with absent preconditions");

    assert_eq!(
        merged["gateway_proxy_headers"],
        json!({ "disabled_hosts": ["new.example.com"] }),
        "present does not match an absent expected section"
    );
    assert_eq!(
        merged["gateway_host_response"],
        json!({ "disabled_hosts": ["compiled.example.com"] }),
        "a section that remains absent may be inserted"
    );
}

#[tokio::test]
async fn config_generation_fence_handles_missing_reset_and_explicit_full_replacements() {
    let (_dir, store) = open_test_store().await;
    let host_a = vec![json!({ "host": "a.example.com" })];
    let host_b = vec![json!({ "host": "b.example.com" })];
    let host_c = vec![json!({ "host": "c.example.com" })];
    let host_d = vec![json!({ "host": "d.example.com" })];
    let groups_d = vec![json!({
        "id": "11111111-1111-4111-8111-111111111111",
        "name": "Imported"
    })];

    store
        .save_config(&json!({
            "host_mappings": host_a,
            "unrelated": { "generation": 1 }
        }))
        .await
        .expect("seed explicit full config");
    // A marker-free value is an intentional full replacement and may update
    // host_mappings while advancing the companion generation.
    store
        .replace_config(&json!({
            "host_mappings": host_b,
            "unrelated": { "generation": 2 }
        }))
        .await
        .expect("explicit full replacement");
    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("2")
    );

    // A snapshot from before a companion-key reset must fail closed even when
    // its host fingerprint still matches the stored config.
    let mut ahead_of_reset = store.get_config().await.unwrap();
    store
        .delete_key(HOST_MAPPINGS_GENERATION_KEY)
        .await
        .unwrap();
    ahead_of_reset["unrelated"]["generation"] = json!(99);
    assert!(store.save_config(&ahead_of_reset).await.is_err());
    assert_eq!(
        store.get_config().await.unwrap()["unrelated"]["generation"],
        json!(2)
    );

    // A fresh read after a missing generation is represented as generation
    // zero. Its host fingerprint still fences the snapshot if the companion
    // key is reset independently after a host change.
    let mut stale_full_config = store.get_config().await.unwrap();
    assert_eq!(
        stale_full_config.pointer(&format!("/{CONFIG_GENERATION_MARKER}/generation")),
        Some(&json!(0))
    );
    store
        .compare_and_set_host_mappings(&host_b, &host_c)
        .await
        .unwrap()
        .expect("host CAS after missing generation");
    store
        .delete_key(HOST_MAPPINGS_GENERATION_KEY)
        .await
        .unwrap();
    stale_full_config["unrelated"]["generation"] = json!(3);
    let stale_save = store.save_config(&stale_full_config).await;
    assert!(stale_save.is_err());
    let after_reset = store.get_config().await.unwrap();
    assert_eq!(after_reset["host_mappings"], json!(host_c));
    assert_eq!(after_reset["unrelated"]["generation"], json!(2));
    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap()
            .as_deref(),
        None
    );

    store
        .replace_config(&json!({
            "host_mappings": host_d,
            "host_mapping_groups": groups_d,
            "unrelated": { "generation": 4 }
        }))
        .await
        .expect("marker-free import remains an explicit replacement");
    let imported = store.get_config().await.unwrap();
    assert_eq!(imported["host_mappings"], json!(host_d));
    assert_eq!(imported["host_mapping_groups"], json!(groups_d));
    assert_eq!(imported["unrelated"]["generation"], json!(4));
    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let persisted_raw = store.get_string_value(CONFIG_KEY).await.unwrap().unwrap();
    assert!(!persisted_raw.contains(CONFIG_GENERATION_MARKER));
}
