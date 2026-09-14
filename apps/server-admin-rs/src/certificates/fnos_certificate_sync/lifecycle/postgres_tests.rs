//! Explicitly opt-in SQL integration tests. No host database or fnOS services are used.
use super::*;

struct Postgres(String);
impl Drop for Postgres {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
    }
}
impl Postgres {
    fn start() -> Self {
        let instance = Self(format!("fn-knock-cert-test-{}", Uuid::new_v4()));
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "-d",
                "--network",
                "none",
                "--name",
                &instance.0,
                "-e",
                "POSTGRES_HOST_AUTH_METHOD=trust",
                "postgres:15",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for _ in 0..120 {
            if instance.query("SELECT 1").is_ok() {
                return instance;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        panic!("isolated PostgreSQL 15 did not start");
    }
    fn query(&self, sql: &str) -> anyhow::Result<String> {
        let mut child = Command::new("docker")
            .args([
                "exec",
                "-i",
                &self.0,
                "psql",
                // The image's temporary initialization server accepts Unix sockets only.
                // TCP readiness ensures we wait for the final server, not that temporary one.
                "-h",
                "127.0.0.1",
                "-U",
                "postgres",
                "-d",
                "postgres",
                "-XqAt",
                "-v",
                "ON_ERROR_STOP=1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(sql.as_bytes())?;
        let result = child.wait_with_output()?;
        anyhow::ensure!(
            result.status.success(),
            "test SQL failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        Ok(String::from_utf8(result.stdout)?)
    }
    fn row(&self) -> Option<Value> {
        serde_json::from_str(
            self.query(
                "SELECT coalesce((SELECT to_jsonb(t) FROM public.cert t WHERE id=8),'null'::jsonb)",
            )
            .unwrap()
            .trim(),
        )
        .unwrap()
    }
    fn apply(&self, rows: &[RowChange], reverse: bool) -> anyhow::Result<String> {
        let verified = schema::inspect(|sql| self.query(sql))?;
        self.query(&row_changes_sql(rows, reverse, &verified)?)
    }
}

#[test]
#[ignore = "starts an isolated, disposable PostgreSQL 15 Docker container"]
fn postgres15_known_schemas_preserve_rows_and_recover() {
    let db = Postgres::start();
    let version = db.query("SHOW server_version_num").unwrap();
    assert_eq!(version.trim().parse::<u32>().unwrap() / 10000, 15);
    for extended in [false, true] {
        db.query("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
            .unwrap();
        let mut ddl = include_str!("fixtures/fnos-certificates.sql").to_string();
        if extended {
            // Reproduce the reported column order as well as the two extra columns.
            ddl = ddl
                .replace("    renewal smallint DEFAULT 0,\n", "")
                .replacen(
                    "    source varchar(16),",
                    "    platform varchar,\n    source varchar(16),\n    cert_url varchar,",
                    1,
                )
                .replacen(
                    "    updated_time bigint\n",
                    "    updated_time bigint,\n    renewal smallint DEFAULT 0\n",
                    1,
                );
        }
        db.query(&ddl).unwrap();
        let verified = schema::inspect(|sql| db.query(sql)).unwrap();
        assert_eq!(
            verified,
            if extended {
                schema::extended()
            } else {
                schema::baseline()
            }
        );
        let (mut snapshot, config) = tests::fixture();
        snapshot.schema = verified.clone();
        let parsed = local_candidates(&config)[0].parsed.clone().unwrap();
        let mut row = prepare_certificate_row(None, &parsed, 8, 10, "create", &verified).unwrap();
        row["certificate"] = json!("/cert");
        row["private_key"] = json!("/key");
        let creation = vec![RowChange {
            table: "cert".into(),
            id: 8,
            before: None,
            after: Some(row.clone()),
        }];
        db.apply(&creation, false).unwrap();
        assert_eq!(
            db.row(),
            Some(row.clone()),
            "complete new row must match PostgreSQL exactly"
        );
        // Simulated journal digest must equal the digest of actual database contents.
        snapshot.rows = vec![row.clone()];
        let expected_digest = target_digest(&snapshot, &row).unwrap();
        tests::managed(&mut snapshot);
        let stored = db.row().unwrap();
        snapshot.rows = vec![stored.clone()];
        assert_eq!(expected_digest, target_digest(&snapshot, &stored).unwrap());
        assert_eq!(
            plan_snapshot(snapshot.clone(), &config).unwrap().actions[0].public["status"],
            "up_to_date"
        );
        db.apply(&creation, true).unwrap();
        db.apply(&creation, true).unwrap(); // Recovery is retryable after a completed DB step.
        assert_eq!(db.row(), None);

        let mut original = tests::fixture().0.rows.remove(0);
        if extended {
            original["platform"] = json!("retain-platform");
            original["cert_url"] = json!("https://example.test/retain");
        }
        db.apply(
            &[RowChange {
                table: "cert".into(),
                id: 8,
                before: None,
                after: Some(original.clone()),
            }],
            false,
        )
        .unwrap();
        db.query("INSERT INTO public.cert_renew(id,cert_id,access_key,access_secret) VALUES(7,8,'test-key','synthetic-secret')").unwrap();
        let renewal: Value = serde_json::from_str(
            db.query("SELECT to_jsonb(t) FROM public.cert_renew t WHERE id=7")
                .unwrap()
                .trim(),
        )
        .unwrap();
        let renewal_change = RowChange {
            table: "cert_renew".into(),
            id: 7,
            before: Some(renewal.clone()),
            after: None,
        };
        for kind in ["update", "adopt"] {
            let after =
                prepare_certificate_row(Some(&original), &parsed, 8, 20, kind, &verified).unwrap();
            if extended {
                assert_eq!(after["platform"], original["platform"]);
                assert_eq!(after["cert_url"], original["cert_url"]);
            }
            let mut change = vec![RowChange {
                table: "cert".into(),
                id: 8,
                before: Some(original.clone()),
                after: Some(after.clone()),
            }];
            change.push(renewal_change.clone());
            db.apply(&change, false).unwrap();
            assert_eq!(db.row(), Some(after.clone()));
            assert_eq!(
                db.query("SELECT count(*) FROM public.cert_renew")
                    .unwrap()
                    .trim(),
                "0"
            );
            // A persisted journal round-trip reproduces recovery after process loss.
            let recovered: Vec<RowChange> =
                serde_json::from_str(&serde_json::to_string(&change).unwrap()).unwrap();
            db.apply(&recovered, true).unwrap();
            let restored_renewal: Value = serde_json::from_str(
                db.query("SELECT to_jsonb(t) FROM public.cert_renew t WHERE id=7")
                    .unwrap()
                    .trim(),
            )
            .unwrap();
            assert_eq!(restored_renewal, renewal);
            assert_eq!(db.row(), Some(original.clone()));
            // External edits stop both application and restoration.
            db.query("UPDATE public.cert SET des='external' WHERE id=8")
                .unwrap();
            assert!(db.apply(&change, false).is_err());
            assert!(db.apply(&change, true).is_err());
            assert_eq!(db.row().unwrap()["des"], "external");
            db.query(&format!(
                "UPDATE public.cert SET des={} WHERE id=8",
                sql_text_expression(original["des"].as_str().unwrap())
            ))
            .unwrap();
        }
        let mut removal = vec![RowChange {
            table: "cert".into(),
            id: 8,
            before: Some(original.clone()),
            after: None,
        }];
        removal.push(renewal_change);
        db.query(
            "INSERT INTO public.cert_used_config(cert_id,service_name) VALUES (8,'test-service')",
        )
        .unwrap();
        assert!(db.apply(&removal, false).is_err());
        assert_eq!(db.row(), Some(original.clone()));
        db.query("DELETE FROM public.cert_used_config").unwrap();
        db.apply(&removal, false).unwrap();
        assert_eq!(db.row(), None);
        db.apply(&removal, true).unwrap();
        assert_eq!(db.row(), Some(original.clone()));

        // An error after a write rolls back the entire DB transaction.
        let mut broken = row_changes_sql(&removal, false, &verified).unwrap();
        broken = broken.strip_suffix("COMMIT;").unwrap().to_string() + "SELECT 1/0; COMMIT;";
        assert!(db.query(&broken).is_err());
        assert_eq!(db.row(), Some(original.clone()));
        // Missing keys in historical journals are not silently equated to SQL NULL.
        if extended {
            let mut incomplete = original.clone();
            incomplete.as_object_mut().unwrap().remove("platform");
            assert!(
                db.apply(
                    &[RowChange {
                        table: "cert".into(),
                        id: 8,
                        before: None,
                        after: Some(incomplete)
                    }],
                    true
                )
                .is_err()
            );
        }
        db.query("CREATE FUNCTION public.cert_test_trigger() RETURNS trigger LANGUAGE plpgsql AS 'BEGIN RETURN NEW; END'; CREATE TRIGGER cert_test BEFORE INSERT ON public.cert FOR EACH ROW EXECUTE FUNCTION public.cert_test_trigger();").unwrap();
        assert!(
            schema::inspect(|sql| db.query(sql))
                .unwrap_err()
                .to_string()
                .contains("triggers")
        );
        db.query("DROP TRIGGER cert_test ON public.cert; ALTER TABLE public.cert ADD COLUMN future varchar").unwrap();
        assert!(
            schema::inspect(|sql| db.query(sql))
                .unwrap_err()
                .to_string()
                .contains("cert.future")
        );
    }
}

#[test]
#[ignore = "starts an isolated, disposable PostgreSQL 15 Docker container"]
fn postgres15_rejects_schema_changes_at_transaction_boundary() {
    let db = Postgres::start();
    for reverse in [false, true] {
        db.query("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
            .unwrap();
        db.query(include_str!("fixtures/fnos-certificates.sql"))
            .unwrap();
        let verified = schema::inspect(|sql| db.query(sql)).unwrap();
        let original = tests::fixture().0.rows.remove(0);
        let change = RowChange {
            table: "cert".into(),
            id: 8,
            before: if reverse {
                Some(original.clone())
            } else {
                None
            },
            after: if reverse {
                None
            } else {
                Some(original.clone())
            },
        };
        // Compile the transaction while preflight still sees the supported schema.
        // For recovery this represents restoring a previously deleted certificate.
        let sql = row_changes_sql(&[change], reverse, &verified).unwrap();
        db.query("CREATE TABLE public.trigger_audit(id bigint); CREATE FUNCTION public.cert_audit() RETURNS trigger LANGUAGE plpgsql AS 'BEGIN INSERT INTO public.trigger_audit VALUES(NEW.id); RETURN NEW; END'; CREATE TRIGGER cert_audit BEFORE INSERT ON public.cert FOR EACH ROW EXECUTE FUNCTION public.cert_audit();").unwrap();
        let error = db.query(&sql).unwrap_err().to_string();
        assert!(error.contains("schema changed"), "{error}");
        assert_eq!(db.row(), None);
        assert_eq!(
            db.query("SELECT count(*) FROM public.trigger_audit")
                .unwrap()
                .trim(),
            "0"
        );
        db.query("DROP TRIGGER cert_audit ON public.cert; ALTER TABLE public.cert ADD COLUMN platform varchar(16), ADD COLUMN cert_url varchar(1024);").unwrap();
        assert!(schema::inspect(|query| db.query(query)).is_ok()); // A supported but different layout.
        assert!(
            db.query(&sql)
                .unwrap_err()
                .to_string()
                .contains("schema changed")
        );
        assert_eq!(db.row(), None);
        db.query("ALTER TABLE public.cert DROP COLUMN platform, DROP COLUMN cert_url; ALTER TABLE public.cert ALTER COLUMN domain TYPE varchar(128);").unwrap();
        assert!(
            db.query(&sql)
                .unwrap_err()
                .to_string()
                .contains("schema changed")
        );
        assert_eq!(db.row(), None);
    }
    // Oversized source/restore data is rejected before the SQL is even sent to PostgreSQL.
    let verified = schema::inspect(|sql| db.query(sql)).unwrap();
    let mut row = tests::fixture().0.rows.remove(0);
    row["domain"] = json!("a".repeat(129));
    for reverse in [false, true] {
        assert!(
            row_changes_sql(
                &[RowChange {
                    table: "cert".into(),
                    id: 8,
                    before: if reverse { Some(row.clone()) } else { None },
                    after: if reverse { None } else { Some(row.clone()) },
                }],
                reverse,
                &verified
            )
            .unwrap_err()
            .to_string()
            .contains("cert.domain")
        );
    }
    assert_eq!(db.row(), None);
    db.query("CREATE DOMAIN public.cert_domain AS varchar(128) CHECK (VALUE <> 'blocked'); ALTER TABLE public.cert ALTER COLUMN domain TYPE public.cert_domain;").unwrap();
    assert!(
        schema::inspect(|sql| db.query(sql))
            .unwrap_err()
            .to_string()
            .contains("domain")
    );
}
