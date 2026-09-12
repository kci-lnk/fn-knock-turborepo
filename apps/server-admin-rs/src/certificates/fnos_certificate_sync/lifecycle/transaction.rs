use super::*;

/// Shared ordering for live mutations and deterministic failure-injection tests.
pub(super) trait Backend {
    fn guard(&mut self, journal: &Journal, reverse: bool) -> anyhow::Result<()>;
    fn verify_restore(&mut self, journal: &Journal) -> anyhow::Result<()>;
    fn check(&mut self, file: &FileChange) -> anyhow::Result<()>;
    fn file(&mut self, file: &FileChange, reverse: bool) -> anyhow::Result<()>;
    fn rows(&mut self, rows: &[RowChange], reverse: bool) -> anyhow::Result<()>;
    fn refresh(&mut self) -> anyhow::Result<()>;
    fn verify(&mut self, journal: &Journal) -> anyhow::Result<()>;
}
pub(super) struct Native<'a> {
    pub data_dir: &'a Path,
    pub selected: &'a [&'a Action],
}
impl Backend for Native<'_> {
    fn guard(&mut self, journal: &Journal, reverse: bool) -> anyhow::Result<()> {
        ensure_removals_unreferenced(journal, &snapshot(self.data_dir)?, reverse)
    }
    fn verify_restore(&mut self, journal: &Journal) -> anyhow::Result<()> {
        verify_restored(journal, self.data_dir)
    }
    fn check(&mut self, file: &FileChange) -> anyhow::Result<()> {
        ensure_parent(Path::new(&file.path), self.data_dir)?;
        check_file(file, true)
    }
    fn file(&mut self, file: &FileChange, reverse: bool) -> anyhow::Result<()> {
        apply_file(file, reverse, self.data_dir)
    }
    fn rows(&mut self, rows: &[RowChange], reverse: bool) -> anyhow::Result<()> {
        apply_rows(rows, reverse)
    }
    fn refresh(&mut self) -> anyhow::Result<()> {
        restart_and_verify_services()
    }
    fn verify(&mut self, journal: &Journal) -> anyhow::Result<()> {
        verify_applied(journal, self.selected, self.data_dir)
    }
}
pub(super) fn apply(journal: &Journal, backend: &mut impl Backend) -> anyhow::Result<()> {
    backend.guard(journal, false)?;
    for file in journal
        .files
        .iter()
        .filter(|f| f.path != NETWORK_CERT_INDEX && f.after.is_some())
    {
        backend.file(file, false)?;
    }
    backend.guard(journal, false)?;
    backend.rows(&journal.rows, false)?;
    for file in journal
        .files
        .iter()
        .filter(|f| f.path == NETWORK_CERT_INDEX)
    {
        backend.file(file, false)?;
    }
    backend.refresh()?;
    for file in journal.files.iter().filter(|f| f.after.is_none()) {
        backend.guard(journal, false)?;
        backend.file(file, false)?;
    }
    backend.verify(journal)?;
    backend.file(&journal.registry, false)?;
    Ok(())
}
pub(super) fn restore(journal: &Journal, backend: &mut impl Backend) -> anyhow::Result<()> {
    backend.guard(journal, true)?;
    for file in journal
        .files
        .iter()
        .chain(std::iter::once(&journal.registry))
    {
        backend.check(file)?;
    }
    backend.rows(&journal.rows, true)?;
    for file in journal.files.iter().rev() {
        if file.before.is_none() {
            backend.guard(journal, true)?;
        }
        backend.file(file, true)?;
    }
    backend.file(&journal.registry, true)?;
    backend.refresh()?;
    backend.verify_restore(journal)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        files: BTreeMap<String, Option<String>>,
        row: Option<Value>,
        fail: Option<&'static str>,
        bind_after_refresh: bool,
        bound: bool,
    }
    impl Fake {
        fn checkpoint(&mut self, name: &str) -> anyhow::Result<()> {
            if self.fail == Some(name) {
                self.fail = None;
                bail!("Injected {name} failure")
            }
            Ok(())
        }
    }
    impl Backend for Fake {
        fn guard(&mut self, journal: &Journal, reverse: bool) -> anyhow::Result<()> {
            self.checkpoint("guard")?;
            if self.bound
                && journal.rows.iter().any(|row| {
                    if reverse {
                        row.before.is_none()
                    } else {
                        row.after.is_none()
                    }
                })
            {
                bail!("Certificate acquired a reference")
            }
            Ok(())
        }
        fn verify_restore(&mut self, _journal: &Journal) -> anyhow::Result<()> {
            self.checkpoint("restore_verify")
        }
        fn check(&mut self, f: &FileChange) -> anyhow::Result<()> {
            let value = self.files.get(&f.path).cloned().flatten();
            if value != f.before && value != f.after {
                bail!("External file change")
            }
            Ok(())
        }
        fn file(&mut self, f: &FileChange, reverse: bool) -> anyhow::Result<()> {
            self.checkpoint(if f.path == NETWORK_CERT_INDEX {
                "index"
            } else {
                "file"
            })?;
            self.check(f)?;
            self.files.insert(
                f.path.clone(),
                if reverse {
                    f.before.clone()
                } else {
                    f.after.clone()
                },
            );
            Ok(())
        }
        fn rows(&mut self, rows: &[RowChange], reverse: bool) -> anyhow::Result<()> {
            self.checkpoint("database")?;
            for row in rows {
                if self.row != row.before && !(reverse && self.row == row.after) {
                    bail!("External database change")
                }
                self.row = if reverse {
                    row.before.clone()
                } else {
                    row.after.clone()
                };
            }
            Ok(())
        }
        fn refresh(&mut self) -> anyhow::Result<()> {
            self.checkpoint("refresh")?;
            self.bound |= self.bind_after_refresh;
            Ok(())
        }
        fn verify(&mut self, _journal: &Journal) -> anyhow::Result<()> {
            self.checkpoint("verify")
        }
    }
    fn fixture(kind: &str) -> (Journal, Fake) {
        let before = if kind == "create" {
            None
        } else {
            Some("old".into())
        };
        let after = if kind == "delete" {
            None
        } else {
            Some("new".into())
        };
        let files = vec![
            FileChange {
                original_metadata: None,
                path: "cert".into(),
                before: before.clone(),
                after: after.clone(),
            },
            FileChange {
                original_metadata: None,
                path: NETWORK_CERT_INDEX.into(),
                before: Some("old-index".into()),
                after: Some("new-index".into()),
            },
        ];
        let registry = FileChange {
            original_metadata: None,
            path: "registry".into(),
            before: Some("old-registry".into()),
            after: Some("new-registry".into()),
        };
        let row_before = before.map(Value::from);
        let row_after = after.map(Value::from);
        let backend = Fake {
            files: files
                .iter()
                .chain(std::iter::once(&registry))
                .map(|f| (f.path.clone(), f.before.clone()))
                .collect(),
            row: row_before.clone(),
            fail: None,
            bind_after_refresh: false,
            bound: false,
        };
        (
            Journal {
                completed: false,
                files,
                rows: vec![RowChange {
                    table: "cert".into(),
                    id: 1,
                    before: row_before,
                    after: row_after,
                }],
                registry,
            },
            backend,
        )
    }
    #[test]
    fn every_failure_stage_restores_create_update_and_delete() {
        for kind in ["create", "update", "delete"] {
            for phase in ["guard", "file", "database", "index", "refresh", "verify"] {
                let (journal, mut backend) = fixture(kind);
                let before = backend.files.clone();
                let row = backend.row.clone();
                backend.fail = Some(phase);
                assert!(apply(&journal, &mut backend).is_err(), "{kind}/{phase}");
                restore(&journal, &mut backend).unwrap();
                restore(&journal, &mut backend).unwrap();
                assert_eq!(backend.files, before, "{kind}/{phase}");
                assert_eq!(backend.row, row, "{kind}/{phase}");
            }
        }
    }
    #[test]
    fn a_reference_added_by_refresh_stops_unlinking_and_is_preserved_on_rollback() {
        let (journal, mut backend) = fixture("delete");
        backend.bind_after_refresh = true;
        assert!(apply(&journal, &mut backend).is_err());
        assert_eq!(backend.files["cert"], journal.files[0].before);
        restore(&journal, &mut backend).unwrap();
        assert!(backend.bound);
        assert_eq!(backend.row, journal.rows[0].before);
        let (created, mut backend) = fixture("create");
        apply(&created, &mut backend).unwrap();
        backend.bound = true;
        assert!(restore(&created, &mut backend).is_err());
        assert_eq!(backend.row, created.rows[0].after);
    }
    #[test]
    fn failed_recovery_verification_is_not_reported_as_success() {
        let (journal, mut backend) = fixture("update");
        apply(&journal, &mut backend).unwrap();
        backend.fail = Some("restore_verify");
        assert!(restore(&journal, &mut backend).is_err());
        restore(&journal, &mut backend).unwrap();
        assert_eq!(backend.row, journal.rows[0].before);
    }
    #[test]
    fn crash_after_database_commit_can_be_recovered_and_external_edits_are_preserved() {
        let (journal, mut backend) = fixture("update");
        backend.file(&journal.files[0], false).unwrap();
        backend.rows(&journal.rows, false).unwrap();
        backend.files.insert("cert".into(), Some("external".into()));
        assert!(restore(&journal, &mut backend).is_err());
        assert_eq!(backend.row, journal.rows[0].after);
        assert_eq!(backend.files["cert"], Some("external".into()));
        backend
            .files
            .insert("cert".into(), journal.files[0].after.clone());
        restore(&journal, &mut backend).unwrap();
        assert_eq!(backend.row, journal.rows[0].before);
    }
}
