//! Persist an interruption/failure latch before any externally visible mutation.
//! A successful explicit recovery/sync is the only way to resume automation.
use super::*;

fn marker(data_dir: &Path) -> PathBuf {
    data_dir.join("fnos-certificate-sync/automatic-pause.json")
}

pub fn automatic_paused(data_dir: &Path) -> anyhow::Result<bool> {
    match fs::symlink_metadata(marker(data_dir)) {
        Ok(_) => {
            validate_fixed_regular_file(&marker(data_dir))?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn mark_attempt(data_dir: &Path) -> anyhow::Result<()> {
    if !automatic_paused(data_dir)? {
        write_content(
            &marker(data_dir),
            &serde_json::to_vec(&json!({
                "version": 1, "started_at": time_utils::now_ms(),
                "reason": "interrupted_or_failed_certificate_operation"
            }))?,
            data_dir,
        )?;
    }
    Ok(())
}

pub(super) fn run<T>(
    data_dir: &Path,
    automatic: bool,
    operation: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let _lock = process_lock(data_dir)?;
    if automatic && automatic_paused(data_dir)? {
        bail!(
            "Automatic certificate synchronization is paused after an interrupted or failed operation; use manual recovery to retry. Backups are preserved"
        )
    }
    let result = operation()?;
    // Keep the cross-process lock until both the operation and latch removal
    // are durable. Errors (including panics/process death) leave the latch set.
    if automatic_paused(data_dir)? {
        fs::remove_file(marker(data_dir))?;
        sync_directory(&data_dir.join("fnos-certificate-sync"))?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn failed_mutation_blocks_timer_notifications_and_restart_until_manual_success() {
        let dir = tempfile::tempdir().unwrap();
        let refreshes = Cell::new(0);
        let attempt = || -> anyhow::Result<()> {
            mark_attempt(dir.path())?;
            refreshes.set(refreshes.get() + 1);
            bail!("verification failed after service refresh")
        };
        assert!(run(dir.path(), true, attempt).is_err());
        for _ in 0..3 {
            assert!(run(dir.path(), true, attempt).is_err());
        }
        assert_eq!(refreshes.get(), 1);
        // A fresh caller reads disk state; no in-memory flag or timer required.
        assert!(automatic_paused(dir.path()).unwrap());
        assert!(run(dir.path(), false, attempt).is_err());
        assert_eq!(refreshes.get(), 2);
        assert!(automatic_paused(dir.path()).unwrap());
        super::super::recover(dir.path()).unwrap();
        assert!(!automatic_paused(dir.path()).unwrap());
        run(dir.path(), true, || Ok(())).unwrap();
    }

    #[test]
    fn interrupted_attempt_is_persistent_and_read_only_failures_do_not_latch() {
        let dir = tempfile::tempdir().unwrap();
        assert!(run::<()>(dir.path(), true, || bail!("invalid preview")).is_err());
        assert!(!automatic_paused(dir.path()).unwrap());
        mark_attempt(dir.path()).unwrap(); // process dies before completion
        assert!(run::<()>(dir.path(), true, || panic!("must not run recovery")).is_err());
        assert!(
            super::super::recover_automatic(dir.path())
                .unwrap_err()
                .to_string()
                .contains("paused")
        );
        assert!(
            super::super::execute_automatic(dir.path(), &json!({}))
                .unwrap_err()
                .to_string()
                .contains("paused")
        );
        assert!(automatic_paused(dir.path()).unwrap());
    }

    #[test]
    fn successful_mutation_clears_latch_and_does_not_delete_transaction_backups() {
        let dir = tempfile::tempdir().unwrap();
        let backup = dir
            .path()
            .join("fnos-certificate-sync/transactions/backup.json");
        fs::create_dir_all(backup.parent().unwrap()).unwrap();
        fs::write(&backup, b"preserve").unwrap();
        run(dir.path(), true, || mark_attempt(dir.path())).unwrap();
        assert!(!automatic_paused(dir.path()).unwrap());
        assert_eq!(fs::read(&backup).unwrap(), b"preserve");
    }
}
