//! Preserve an externally deleted certificate instead of resurrecting it from an
//! obsolete journal. Only explicit recovery reaches this path. No live fnOS state
//! is written, and retired journals remain outside the completed-backup pruner.
use super::*;

fn candidate(journal: &Journal) -> bool {
    !journal.completed
        && journal
            .rows
            .iter()
            .any(|row| row.table == "cert" && row.before.is_some())
        && journal
            .rows
            .iter()
            .filter(|row| row.table == "cert")
            .all(|row| row.after.is_some())
        && journal.files.iter().all(|file| file.after.is_some())
}

fn keyed(index: &[Value]) -> Option<BTreeMap<String, Value>> {
    let mut entries = BTreeMap::new();
    for entry in index {
        let path = entry["certificate"]
            .as_str()
            .filter(|path| !path.is_empty())?;
        if entries.insert(path.to_string(), entry.clone()).is_some() {
            return None;
        }
    }
    Some(entries)
}

fn recorded_paths(row: &Value, index: &[Value]) -> BTreeSet<String> {
    let mut paths = file_paths(row, index);
    for entry in index
        .iter()
        .filter(|entry| entry["certificate"] == row["certificate"])
    {
        paths.extend(index_file_paths(entry).map(str::to_owned));
    }
    paths
}

fn eligible(
    journal: &Journal,
    current: &Snapshot,
    mut missing: impl FnMut(&str) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    if !candidate(journal) {
        return Ok(false);
    }
    current.schema.verify_changes(&journal.rows)?;
    let index_changes = journal
        .files
        .iter()
        .filter(|file| file.path == NETWORK_CERT_INDEX)
        .collect::<Vec<_>>();
    let [change] = index_changes.as_slice() else {
        return Ok(false);
    };
    let decode = |value: &Option<String>| -> anyhow::Result<Vec<Value>> {
        Ok(serde_json::from_slice(
            &BASE64_STANDARD.decode(
                value
                    .as_ref()
                    .ok_or_else(|| anyhow!("Missing index recovery record"))?,
            )?,
        )?)
    };
    let before = decode(&change.before)?;
    let after = decode(&change.after)?;
    let (Some(mut before_entries), Some(mut after_entries), Some(current_entries)) =
        (keyed(&before), keyed(&after), keyed(&current.index))
    else {
        return Ok(false);
    };
    let mut ids = BTreeSet::new();
    let mut affected = BTreeSet::new();
    let mut original_files = BTreeSet::new();
    for row in journal.rows.iter().filter(|row| row.table == "cert") {
        if !ids.insert(row.id) {
            return Ok(false);
        }
        let Some(after_row) = &row.after else {
            return Ok(false);
        };
        let Some(path) = after_row["certificate"]
            .as_str()
            .filter(|path| !path.is_empty())
        else {
            return Ok(false);
        };
        if after_entries.remove(path).is_none() {
            return Ok(false);
        }
        affected.extend(recorded_paths(after_row, &after));
        if let Some(before_row) = &row.before {
            // Updates do not unlink old files. Their disappearance is evidence of
            // an external deletion, unlike a partial rollback of a new certificate.
            if before_row["certificate"] != after_row["certificate"]
                || before_entries.remove(path).is_none()
                || recorded_paths(before_row, &before) != recorded_paths(after_row, &after)
            {
                return Ok(false);
            }
            original_files.extend(recorded_paths(before_row, &before));
        } else if before_entries.contains_key(path) {
            return Ok(false);
        }
    }
    // Unrelated entries must survive unchanged. Treat order as irrelevant here;
    // this decision never rewrites the index or relaxes normal rollback checks.
    if before_entries != after_entries
        || before_entries != current_entries
        || original_files.is_empty()
    {
        return Ok(false);
    }
    let row_paths = current
        .rows
        .iter()
        .filter_map(|row| row["certificate"].as_str())
        .collect::<BTreeSet<_>>();
    if row_paths.len() != current.rows.len()
        || row_paths != current_entries.keys().map(String::as_str).collect()
    {
        return Ok(false);
    }
    // A missing certificate store or an inconsistent surviving certificate is not
    // evidence of intentional deletion. Require readable live files and agreement
    // between the remaining database and index keys before retiring a journal.
    for row in &current.rows {
        let entry = &current_entries[row["certificate"].as_str().unwrap()];
        if entry["privateKey"] != row["private_key"]
            || row["private_key"]
                .as_str()
                .is_none_or(|path| path.is_empty())
            || recorded_paths(row, &current.index)
                .iter()
                .any(|path| !current.files.get(path).is_some_and(Option::is_some))
        {
            return Ok(false);
        }
    }
    // Preparation only removes renewal settings belonging to selected certs.
    // Do not retire unrelated row operations, even if their IDs are now absent.
    let mut renewal_ids = BTreeSet::new();
    for row in journal.rows.iter().filter(|row| row.table == "cert_renew") {
        if !renewal_ids.insert(row.id)
            || row.after.is_some()
            || !row.before.as_ref().is_some_and(|before| {
                before["cert_id"]
                    .as_i64()
                    .is_some_and(|id| ids.contains(&id))
            })
        {
            return Ok(false);
        }
    }
    if current
        .rows
        .iter()
        .any(|row| row["id"].as_i64().is_some_and(|id| ids.contains(&id)))
        || current
            .used
            .iter()
            .chain(&current.renew)
            .any(|row| row["cert_id"].as_i64().is_some_and(|id| ids.contains(&id)))
        || journal
            .rows
            .iter()
            .filter(|row| row.table == "cert_renew")
            .any(|row| {
                current
                    .renew
                    .iter()
                    .any(|live| live["id"].as_i64() == Some(row.id))
            })
    {
        return Ok(false);
    }
    for file in journal
        .files
        .iter()
        .filter(|file| file.path != NETWORK_CERT_INDEX)
    {
        if !affected.contains(&file.path) {
            return Ok(false);
        }
    }
    // Include shared keys/chains and SNI entries, even mappings we would normally
    // recognize as generated. Retirement must not strand any active consumer.
    if current
        .rows
        .iter()
        .any(|row| !file_paths(row, &current.index).is_disjoint(&affected))
        || current
            .index
            .iter()
            .flat_map(index_file_paths)
            .any(|path| affected.contains(path))
        || current.gateway.iter().any(|entry| {
            ["cert", "key"].iter().any(|key| {
                entry[key]
                    .as_str()
                    .is_some_and(|path| affected.contains(path))
            })
        })
    {
        return Ok(false);
    }
    for path in &original_files {
        // Adoption journals have a database/index before-image but do not rewrite
        // (or back up) certificate files. They can become obsolete in the same way.
        // A recorded absent before-image, however, is not evidence of an old file.
        if journal
            .files
            .iter()
            .any(|file| &file.path == path && file.before.is_none())
            || !missing(path)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn revision(snapshot: &Snapshot) -> anyhow::Result<String> {
    digest(&(
        &snapshot.rows,
        &snapshot.used,
        &snapshot.renew,
        &snapshot.index,
        &snapshot.gateway,
        &snapshot.files,
        &snapshot.registry_bytes,
    ))
}

pub(super) fn try_retire(
    path: &Path,
    journal: &Journal,
    bytes: &[u8],
    data_dir: &Path,
) -> anyhow::Result<bool> {
    try_retire_using(
        path,
        journal,
        bytes,
        data_dir,
        || snapshot(data_dir),
        |path| original_file_missing(Path::new(path), data_dir),
    )
}

fn original_file_missing(path: &Path, data_dir: &Path) -> anyhow::Result<bool> {
    validate_write_location(path, data_dir)?;
    // Missing ancestors are allowed for writes, but the entire store disappearing
    // must not be interpreted as deletion of this individual certificate.
    let root = if path.starts_with(CERT_ROOT) {
        PathBuf::from(CERT_ROOT)
    } else {
        data_dir.join("fnos-certificate-sync")
    };
    let metadata = fs::symlink_metadata(root)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Certificate store unavailable; recovery backup preserved"
    );
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false), // Including dangling links; never treat them as absent.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error.into()),
    }
}

fn try_retire_using(
    path: &Path,
    journal: &Journal,
    bytes: &[u8],
    data_dir: &Path,
    mut snapshot: impl FnMut() -> anyhow::Result<Snapshot>,
    mut missing: impl FnMut(&str) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    if !candidate(journal) {
        return Ok(false);
    }
    let current = snapshot()?;
    if !eligible(journal, &current, &mut missing)? {
        return Ok(false);
    }
    let checked = snapshot()?;
    anyhow::ensure!(
        revision(&current)? == revision(&checked)? && eligible(journal, &checked, &mut missing)?,
        "Certificate state changed while checking obsolete recovery; backup preserved"
    );
    archive_using(path, bytes, data_dir, |destination| {
        write_content_with_metadata(
            destination,
            bytes,
            data_dir,
            Some(&SavedMetadata::read(path)?),
        )
    })?;
    Ok(true)
}

fn archive_using(
    path: &Path,
    bytes: &[u8],
    data_dir: &Path,
    write_backup: impl FnOnce(&Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        path.parent()
            == Some(
                data_dir
                    .join("fnos-certificate-sync/transactions")
                    .as_path()
            ),
        "Invalid recovery journal location"
    );
    validate_write_location(path, data_dir)?;
    validate_fixed_regular_file(path)?;
    anyhow::ensure!(
        fs::read(path)? == bytes,
        "Certificate recovery journal changed; backup preserved"
    );
    let destination = data_dir
        .join("fnos-certificate-sync/retired-transactions")
        .join(Uuid::new_v4().to_string())
        .join(
            path.file_name()
                .ok_or_else(|| anyhow!("Missing recovery journal name"))?,
        );
    // Commit a separate durable copy before removing the recovery entry. Unlike
    // rename followed by fsync, a backup write/fsync failure retains the pending
    // journal. Interruption may leave two copies, which is safe to retry.
    write_backup(&destination)?;
    validate_fixed_regular_file(path)?;
    anyhow::ensure!(
        fs::read(path)? == bytes,
        "Certificate recovery journal changed during archival; backup preserved"
    );
    fs::remove_file(path)?;
    sync_directory(
        path.parent()
            .ok_or_else(|| anyhow!("Missing recovery directory"))?,
    )?;
    tracing::warn!(event = "certificate_transaction_retired", backup = %destination.display(),
        "Archived obsolete certificate transaction; external deletions and live state preserved");
    Ok(())
}

#[cfg(test)]
mod tests;
