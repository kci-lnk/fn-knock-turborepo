use super::*;

fn encoded(value: &impl Serialize) -> Option<String> {
    Some(BASE64_STANDARD.encode(serde_json::to_vec(value).unwrap()))
}

// Reported incident: three original entries, update one + create three, then
// only the two untouched entries survive. The original updated cert/key are
// missing; newly created files may still exist. No gateway snapshot is available.
fn fixture() -> (Journal, Snapshot) {
    let (mut current, _) = super::super::tests::fixture();
    let template_row = current.rows[0].clone();
    let template_index = current.index[0].clone();
    let pair = |id: i64| {
        let mut row = template_row.clone();
        row["id"] = json!(id);
        row["certificate"] = json!(format!("/cert-{id}"));
        row["private_key"] = json!(format!("/key-{id}"));
        let mut index = template_index.clone();
        index["certificate"] = row["certificate"].clone();
        index["privateKey"] = row["private_key"].clone();
        (row, index)
    };
    let retained = [pair(1), pair(2)];
    current.rows = retained.iter().map(|p| p.0.clone()).collect();
    current.index = retained.iter().map(|p| p.1.clone()).collect();
    current.files = current
        .rows
        .iter()
        .flat_map(|row| file_paths(row, &current.index))
        .map(|path| (path, Some(BASE64_STANDARD.encode(b"live certificate file"))))
        .collect();
    let mut before = current.index.clone();
    before.push(pair(8).1);
    let mut after = before.clone();
    let mut rows = Vec::new();
    let mut files = Vec::new();
    for id in 8..12 {
        let (row, index) = pair(id);
        if id != 8 {
            after.push(index)
        }
        for path in file_paths(&row, &after) {
            files.push(FileChange {
                path,
                before: (id == 8).then(|| BASE64_STANDARD.encode(b"original")),
                after: Some(BASE64_STANDARD.encode(b"applied")),
                original_metadata: None,
                applied_metadata: None,
            });
        }
        rows.push(RowChange {
            table: "cert".into(),
            id,
            before: (id == 8).then(|| row.clone()),
            after: Some(row),
        });
    }
    files.push(FileChange {
        path: NETWORK_CERT_INDEX.into(),
        before: encoded(&before),
        after: encoded(&after),
        original_metadata: None,
        applied_metadata: None,
    });
    (
        Journal {
            gateway_before: None,
            directories: vec![],
            completed: false,
            files,
            rows,
            registry: FileChange {
                path: "registry".into(),
                before: None,
                after: encoded(&Registry::default()),
                original_metadata: None,
                applied_metadata: None,
            },
        },
        current,
    )
}

#[test]
fn recognizes_the_reported_legacy_transaction_without_reading_new_private_keys() {
    let (journal, current) = fixture();
    let mut inspected = BTreeSet::new();
    assert!(
        eligible(&journal, &current, |path| {
            inspected.insert(path.to_owned());
            Ok(true)
        })
        .unwrap()
    );
    assert_eq!(
        inspected,
        BTreeSet::from(["/cert-8".into(), "/key-8".into()])
    );
    let mut reversed = current.clone();
    reversed.index.reverse();
    assert!(eligible(&journal, &reversed, |_| Ok(true)).unwrap());
}

#[test]
fn normal_interrupted_creation_update_and_deletion_still_require_rollback() {
    let (journal, current) = fixture();
    for present in ["/cert-8", "/key-8"] {
        assert!(!eligible(&journal, &current, |path| Ok(path != present)).unwrap());
    }
    let mut creation = journal.clone();
    for row in &mut creation.rows {
        row.before = None;
    }
    assert!(
        !eligible(&creation, &current, |_| panic!(
            "creation must not inspect files"
        ))
        .unwrap()
    );
    let mut deletion = journal.clone();
    deletion.rows[0].after = None;
    assert!(
        !eligible(&deletion, &current, |_| panic!(
            "deletion must not inspect files"
        ))
        .unwrap()
    );
    let mut unbacked = journal.clone();
    unbacked
        .files
        .iter_mut()
        .find(|file| file.path == "/key-8")
        .unwrap()
        .before = None;
    assert!(!eligible(&unbacked, &current, |_| Ok(true)).unwrap());
    let mut completed = journal.clone();
    completed.completed = true;
    assert!(!eligible(&completed, &current, |_| Ok(true)).unwrap());
}

#[test]
fn externally_deleted_adoptions_do_not_require_certificate_file_backups() {
    let (mut journal, current) = fixture();
    journal
        .files
        .retain(|file| file.path != "/cert-8" && file.path != "/key-8");
    assert!(eligible(&journal, &current, |_| Ok(true)).unwrap());
    assert!(!eligible(&journal, &current, |_| Ok(false)).unwrap());
}

#[test]
fn any_surviving_row_id_binding_renewal_or_shared_path_blocks_retirement() {
    let (journal, current) = fixture();
    for id in 8..12 {
        let mut live = current.clone();
        live.rows[0]["id"] = json!(id); // Reused ID, even at another path.
        assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
        live = current.clone();
        live.used.push(json!({"cert_id":id}));
        assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
        live = current.clone();
        live.renew.push(json!({"id":50,"cert_id":id}));
        assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
    }
    for path in ["/cert-8", "/key-8", "/cert-9", "/key-11"] {
        for field in ["private_key", "issuer_certificate"] {
            let mut live = current.clone();
            live.rows[0][field] = json!(path);
            assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
        }
        for field in ["cert", "key"] {
            let mut live = current.clone();
            live.gateway.push(json!({field:path}));
            assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
        }
    }
}

#[test]
fn unrelated_changes_duplicate_indices_and_partial_database_deletion_are_not_accepted() {
    let (journal, current) = fixture();
    let mut live = current.clone();
    live.index[0]["used"] = json!(true);
    assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
    live = current.clone();
    live.index.push(live.index[0].clone());
    assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
    live = current.clone();
    live.rows.pop();
    assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
    let mut malformed = journal.clone();
    malformed.rows[0]
        .after
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("source");
    assert!(eligible(&malformed, &current, |_| Ok(true)).is_err());
    assert!(eligible(&journal, &current, |_| bail!("injected permission failure")).is_err());
}

fn pending(dir: &Path, journal: &Journal) -> (PathBuf, Vec<u8>) {
    let path = dir.join("fnos-certificate-sync/transactions/legacy.json");
    let mut value = serde_json::to_value(journal).unwrap();
    value.as_object_mut().unwrap().remove("gateway_before");
    value["future_audit_field"] = json!("preserve unknown fields and original formatting");
    let bytes = serde_json::to_vec_pretty(&value).unwrap();
    write_content(&path, &bytes, dir).unwrap();
    (path, bytes)
}

#[test]
fn explicit_recovery_archives_original_bytes_clears_pause_and_preserves_live_files() {
    let dir = tempfile::tempdir().unwrap();
    let (journal, current) = fixture();
    let (path, bytes) = pending(dir.path(), &journal);
    let live_file = dir.path().join("residual-private-key");
    fs::write(&live_file, b"retained-secret").unwrap();
    write_content(&registry_path(dir.path()), b"{\"entries\":{}}", dir.path()).unwrap();
    automatic::mark_attempt(dir.path()).unwrap();
    // Neither startup nor timer work may bypass the persistent manual-recovery latch.
    assert!(recover_automatic(dir.path()).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    automatic::run(dir.path(), false, || {
        assert!(try_retire_using(
            &path,
            &journal,
            &bytes,
            dir.path(),
            || Ok(current.clone()),
            |_| Ok(true)
        )?);
        Ok(())
    })
    .unwrap();
    assert!(!pending_recovery(dir.path()).unwrap());
    assert!(!automatic_paused(dir.path()).unwrap());
    assert!(!recover(dir.path()).unwrap()); // Repeating recovery is a no-op, not a rollback.
    prune_transactions(dir.path()).unwrap();
    let archive_dir = fs::read_dir(
        dir.path()
            .join("fnos-certificate-sync/retired-transactions"),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap()
    .path();
    assert_eq!(fs::read(archive_dir.join("legacy.json")).unwrap(), bytes);
    assert_eq!(fs::read(live_file).unwrap(), b"retained-secret");
    assert_eq!(
        fs::read(registry_path(dir.path())).unwrap(),
        b"{\"entries\":{}}"
    );
    #[cfg(unix)]
    assert_eq!(fs::metadata(archive_dir).unwrap().mode() & 0o777, 0o700);
    assert!(plan_snapshot(current, &json!({"ssl":{"certificates":[]}})).is_ok());
}

#[test]
fn state_or_journal_change_during_inspection_keeps_the_journal_and_pause() {
    for fault in ["database", "file", "journal", "destination"] {
        let dir = tempfile::tempdir().unwrap();
        let (journal, current) = fixture();
        let (path, bytes) = pending(dir.path(), &journal);
        automatic::mark_attempt(dir.path()).unwrap();
        if fault == "destination" {
            fs::write(
                dir.path()
                    .join("fnos-certificate-sync/retired-transactions"),
                b"blocked",
            )
            .unwrap();
        }
        let mut reads = 0;
        let mut file_reads = 0;
        let result = automatic::run(dir.path(), false, || {
            try_retire_using(
                &path,
                &journal,
                &bytes,
                dir.path(),
                || {
                    reads += 1;
                    let mut snapshot = current.clone();
                    if reads == 2 {
                        if fault == "database" {
                            snapshot.used.push(json!({"cert_id":9}));
                        }
                        if fault == "journal" {
                            fs::write(&path, b"external journal edit")?;
                        }
                    }
                    Ok(snapshot)
                },
                |_| {
                    file_reads += 1;
                    Ok(fault != "file" || file_reads <= 2)
                },
            )
        });
        assert!(result.is_err(), "{fault}");
        assert!(path.exists(), "{fault}");
        assert!(automatic_paused(dir.path()).unwrap(), "{fault}");
        if fault != "journal" {
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
}

#[test]
fn interruption_after_archive_is_retryable_without_replaying_the_old_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let (journal, current) = fixture();
    let (path, bytes) = pending(dir.path(), &journal);
    automatic::mark_attempt(dir.path()).unwrap();
    let result: anyhow::Result<()> = automatic::run(dir.path(), false, || {
        assert!(try_retire_using(
            &path,
            &journal,
            &bytes,
            dir.path(),
            || Ok(current.clone()),
            |_| Ok(true)
        )?);
        bail!("injected interruption before clearing the pause marker")
    });
    assert!(result.is_err());
    assert!(automatic_paused(dir.path()).unwrap());
    assert!(!pending_recovery(dir.path()).unwrap());
    assert!(recover_automatic(dir.path()).is_err());
    assert!(!recover(dir.path()).unwrap());
    assert!(!automatic_paused(dir.path()).unwrap());
}

#[test]
fn missing_file_evidence_requires_safe_parents_and_does_not_follow_links() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fnos-certificate-sync");
    let path = root.join("certificate");
    assert!(original_file_missing(&path, dir.path()).is_err());
    fs::create_dir(&root).unwrap();
    assert!(original_file_missing(&path, dir.path()).unwrap());
    fs::write(&path, b"retained").unwrap();
    assert!(!original_file_missing(&path, dir.path()).unwrap());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(!original_file_missing(&path, dir.path()).unwrap());
    assert!(original_file_missing(&dir.path().join("outside-root"), dir.path()).is_err());
    #[cfg(unix)]
    {
        let link = root.join("dangling");
        std::os::unix::fs::symlink(root.join("missing"), &link).unwrap();
        assert!(!original_file_missing(&link, dir.path()).unwrap());
        assert!(original_file_missing(&link.join("child"), dir.path()).is_err());
    }
}

#[test]
fn reusing_an_old_renewal_id_and_incompatible_schema_also_block_retirement() {
    let (mut journal, mut current) = fixture();
    let renewal = json!({"id":50,"cert_id":8,"verify_type":null,"renew_email":null,
        "platform":"letsencrypt","ddns_provider":null,"access_key":null,"access_secret":null,
        "renew_cert_url":null,"ddns_related_id":0,"last_renew_time":null,"last_renew_error":null,
        "created_time":null,"updated_time":null});
    journal.rows.push(RowChange {
        table: "cert_renew".into(),
        id: 50,
        before: Some(renewal),
        after: None,
    });
    assert!(eligible(&journal, &current, |_| Ok(true)).unwrap());
    current.renew.push(json!({"id":50,"cert_id":1}));
    assert!(!eligible(&journal, &current, |_| Ok(true)).unwrap());
    current.renew.clear();
    current.schema = schema::extended();
    assert!(eligible(&journal, &current, |_| Ok(true)).is_err());
}

#[test]
fn missing_live_files_and_mismatched_live_keys_do_not_prove_external_deletion() {
    let (journal, current) = fixture();
    let mut live = current.clone();
    live.files.insert("/cert-1".into(), None);
    assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
    live = current.clone();
    live.files.remove("/key-2");
    assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
    live = current.clone();
    live.rows[0]["private_key"] = json!("/another-key");
    live.files
        .insert("/another-key".into(), Some(BASE64_STANDARD.encode(b"key")));
    assert!(!eligible(&journal, &live, |_| Ok(true)).unwrap());
}

#[test]
fn historical_index_private_keys_are_part_of_deletion_and_reference_checks() {
    let (mut journal, mut current) = fixture();
    let change = journal
        .files
        .iter_mut()
        .find(|file| file.path == NETWORK_CERT_INDEX)
        .unwrap();
    for value in [&mut change.before, &mut change.after] {
        let mut entries: Vec<Value> =
            serde_json::from_slice(&BASE64_STANDARD.decode(value.as_ref().unwrap()).unwrap())
                .unwrap();
        entries
            .iter_mut()
            .find(|e| e["certificate"] == "/cert-8")
            .unwrap()["privateKey"] = json!("/historical-index-key");
        *value = encoded(&entries);
    }
    assert!(
        !eligible(&journal, &current, |path| Ok(
            path != "/historical-index-key"
        ))
        .unwrap()
    );
    current
        .gateway
        .push(json!({"host":"example.test","cert":"/other","key":"/historical-index-key"}));
    assert!(!eligible(&journal, &current, |_| Ok(true)).unwrap());
}

#[test]
fn unrelated_renewal_changes_cannot_be_discarded_with_deleted_certificates() {
    let (mut journal, current) = fixture();
    let renewal = json!({"id":50,"cert_id":1,"verify_type":null,"renew_email":null,
        "platform":"letsencrypt","ddns_provider":null,"access_key":null,"access_secret":null,
        "renew_cert_url":null,"ddns_related_id":0,"last_renew_time":null,"last_renew_error":null,
        "created_time":null,"updated_time":null});
    journal.rows.push(RowChange {
        table: "cert_renew".into(),
        id: 50,
        before: Some(renewal),
        after: None,
    });
    assert!(!eligible(&journal, &current, |_| Ok(true)).unwrap());
}

#[test]
fn archive_failures_leave_a_recoverable_journal_and_pause() {
    for fault in ["write", "flush", "source_changed"] {
        let dir = tempfile::tempdir().unwrap();
        let (journal, _) = fixture();
        let (path, bytes) = pending(dir.path(), &journal);
        automatic::mark_attempt(dir.path()).unwrap();
        let mut copy = None;
        let result = automatic::run(dir.path(), false, || {
            archive_using(&path, &bytes, dir.path(), |destination| {
                if fault == "write" {
                    bail!("injected backup write failure");
                }
                write_content(destination, &bytes, dir.path())?;
                copy = Some(destination.to_path_buf());
                if fault == "flush" {
                    bail!("injected backup directory fsync failure");
                }
                fs::write(&path, b"external journal change")?;
                Ok(())
            })
        });
        assert!(result.is_err(), "{fault}");
        assert!(automatic_paused(dir.path()).unwrap(), "{fault}");
        let expected: &[u8] = if fault == "source_changed" {
            b"external journal change"
        } else {
            &bytes
        };
        assert_eq!(fs::read(&path).unwrap(), expected, "{fault}");
        if let Some(copy) = copy {
            assert_eq!(fs::read(copy).unwrap(), bytes);
        }
    }
}
