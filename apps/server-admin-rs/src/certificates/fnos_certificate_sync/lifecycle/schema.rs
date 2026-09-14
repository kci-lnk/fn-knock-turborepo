//! Only explicitly supported fnOS layouts may reach certificate mutation/recovery.
use super::*;

const COLUMNS_SQL: &str = r#"SELECT coalesce(jsonb_agg(t ORDER BY table_name COLLATE "C",column_name COLLATE "C"),'[]'::jsonb) FROM (SELECT table_name,column_name,data_type,is_nullable,column_default,is_identity,is_generated,character_maximum_length,domain_name FROM information_schema.columns WHERE table_schema='public' AND table_name IN ('cert','cert_used_config','cert_renew')) t"#;
const TRIGGERS_SQL: &str = "SELECT count(*) FROM pg_trigger WHERE tgrelid IN ('public.cert'::regclass,'public.cert_renew'::regclass,'public.cert_used_config'::regclass)";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Column {
    table_name: String,
    column_name: String,
    data_type: String,
    is_nullable: String,
    // Missing metadata must not be treated as SQL NULL.
    #[serde(deserialize_with = "required_nullable_string")]
    column_default: Option<String>,
    is_identity: String,
    is_generated: String,
    #[serde(deserialize_with = "required_nullable_length")]
    character_maximum_length: Option<i64>,
    #[serde(deserialize_with = "required_nullable_string")]
    domain_name: Option<String>,
}
fn required_nullable_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

fn required_nullable_length<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    Option::<i64>::deserialize(deserializer)
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct VerifiedSchema {
    // Canonical order makes physical column ordering irrelevant to plan identity.
    columns: Vec<Column>,
}
impl VerifiedSchema {
    // Must run after LOCK TABLE and in the same transaction as all mutations.
    // Compare metadata again so preflight inspection cannot race DDL or CREATE TRIGGER.
    pub(super) fn transaction_guard_sql(&self) -> anyhow::Result<String> {
        let expected = json_sql(&serde_json::to_value(&self.columns)?);
        Ok(format!(
            "SELECT CASE WHEN ({COLUMNS_SQL}) = {expected} AND ({TRIGGERS_SQL}) = 0 THEN 1 ELSE (random()::text || 'fnOS certificate schema changed')::integer END;\n"
        ))
    }
    pub(super) fn verify_changes(&self, changes: &[RowChange]) -> anyhow::Result<()> {
        for change in changes {
            if !matches!(change.table.as_str(), "cert" | "cert_renew") {
                bail!("Invalid recovery table")
            }
            for row in [&change.before, &change.after].into_iter().flatten() {
                self.verify_row(&change.table, row)?;
                if row["id"].as_i64() != Some(change.id) {
                    bail!("Certificate journal row identity mismatch; recovery backup preserved")
                }
            }
        }
        Ok(())
    }
    fn verify_row(&self, table: &str, row: &Value) -> anyhow::Result<()> {
        let expected: BTreeSet<_> = self
            .columns
            .iter()
            .filter(|c| c.table_name == table)
            .map(|c| c.column_name.as_str())
            .collect();
        let actual = row
            .as_object()
            .ok_or_else(|| anyhow!("Invalid certificate row"))?;
        if expected.is_empty()
            || actual.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
        {
            bail!(
                "Unsupported fnOS certificate database structure: {table} journal row fields differ; recovery backup preserved"
            )
        }
        for column in self.columns.iter().filter(|c| c.table_name == table) {
            let value = &actual[&column.column_name];
            let valid = if value.is_null() {
                column.is_nullable == "YES"
            } else {
                match column.data_type.as_str() {
                    "character varying" => value.as_str().is_some_and(|s| {
                        !s.contains('\0')
                            && column
                                .character_maximum_length
                                .is_none_or(|max| s.chars().count() as i64 <= max)
                    }),
                    "bigint" => value.as_i64().is_some(),
                    "smallint" => value.as_i64().is_some_and(|v| i16::try_from(v).is_ok()),
                    _ => false,
                }
            };
            if !valid {
                bail!(
                    "Certificate row incompatible with {table}.{} type, nullability or length; recovery backup preserved",
                    column.column_name
                )
            }
        }
        Ok(())
    }
    pub(super) fn complete_new_cert(&self, row_value: &mut Value) -> anyhow::Result<()> {
        let row = row_value
            .as_object_mut()
            .ok_or_else(|| anyhow!("Invalid new certificate row"))?;
        for column in self.columns.iter().filter(|c| c.table_name == "cert") {
            if matches!(column.column_name.as_str(), "platform" | "cert_url") {
                row.insert(column.column_name.clone(), Value::Null);
            }
        }
        self.verify_row("cert", row_value)
    }
}

pub(super) fn inspect(
    mut query: impl FnMut(&str) -> anyhow::Result<String>,
) -> anyhow::Result<VerifiedSchema> {
    let columns: Vec<Column> = serde_json::from_str(&query(COLUMNS_SQL)?).map_err(|_| {
        anyhow!("Unsupported fnOS certificate database structure: invalid column metadata")
    })?;
    let schema = validate(columns)?;
    if query(TRIGGERS_SQL)?.trim() != "0" {
        bail!("Unsupported fnOS certificate database triggers")
    }
    Ok(schema)
}

fn validate(mut columns: Vec<Column>) -> anyhow::Result<VerifiedSchema> {
    columns.sort_by(|a, b| (&a.table_name, &a.column_name).cmp(&(&b.table_name, &b.column_name)));
    let extended = columns.iter().any(|c| {
        c.table_name == "cert" && matches!(c.column_name.as_str(), "platform" | "cert_url")
    });
    let mut expected = baseline().columns;
    if extended {
        for name in ["platform", "cert_url"] {
            expected.push(column("cert", name, "character varying", "YES", None));
        }
    }
    let actual: BTreeMap<_, _> = columns
        .iter()
        .map(|c| ((c.table_name.as_str(), c.column_name.as_str()), c))
        .collect();
    if actual.len() != columns.len() {
        bail!("Unsupported fnOS certificate database structure: duplicate column metadata")
    }
    for wanted in &expected {
        let Some(found) = actual.get(&(wanted.table_name.as_str(), wanted.column_name.as_str()))
        else {
            bail!(
                "Unsupported fnOS certificate database structure: {}.{} missing",
                wanted.table_name,
                wanted.column_name
            )
        };
        for (attribute, matches) in [
            ("data_type", found.data_type == wanted.data_type),
            ("is_nullable", found.is_nullable == wanted.is_nullable),
            (
                "column_default",
                found.column_default == wanted.column_default,
            ),
            ("is_identity", found.is_identity == wanted.is_identity),
            ("is_generated", found.is_generated == wanted.is_generated),
        ] {
            if !matches {
                // Never include actual defaults: expressions could contain sensitive literals.
                bail!(
                    "Unsupported fnOS certificate database structure: {}.{} incompatible {}",
                    wanted.table_name,
                    wanted.column_name,
                    attribute
                )
            }
        }
        if found.domain_name.is_some()
            || (found.data_type == "character varying"
                && found.character_maximum_length.is_some_and(|max| max <= 0))
            || (found.data_type != "character varying" && found.character_maximum_length.is_some())
        {
            bail!(
                "Unsupported fnOS certificate database structure: {}.{} incompatible domain or length metadata",
                wanted.table_name,
                wanted.column_name
            )
        }
    }
    let expected_keys: BTreeSet<_> = expected
        .iter()
        .map(|c| (&c.table_name, &c.column_name))
        .collect();
    for c in &columns {
        if !expected_keys.contains(&(&c.table_name, &c.column_name)) {
            bail!(
                "Unsupported fnOS certificate database structure: {}.{} unexpected column",
                c.table_name,
                c.column_name
            )
        }
    }
    Ok(VerifiedSchema { columns })
}
fn column(
    table: &str,
    name: &str,
    data_type: &str,
    nullable: &str,
    default: Option<&str>,
) -> Column {
    Column {
        table_name: table.into(),
        column_name: name.into(),
        data_type: data_type.into(),
        is_nullable: nullable.into(),
        column_default: default.map(str::to_string),
        is_identity: "NO".into(),
        is_generated: "NEVER".into(),
        character_maximum_length: None,
        domain_name: None,
    }
}
pub(super) fn baseline() -> VerifiedSchema {
    let mut columns = Vec::new();
    for table in ["cert", "cert_renew", "cert_used_config"] {
        let sequence = format!("nextval('{table}_id_seq'::regclass)");
        columns.push(column(table, "id", "bigint", "NO", Some(&sequence)));
        for name in ["created_time", "updated_time"] {
            columns.push(column(table, name, "bigint", "YES", None));
        }
        if table != "cert" {
            columns.push(column(table, "cert_id", "bigint", "NO", None));
        }
    }
    columns.push(column("cert", "domain", "character varying", "NO", None));
    for name in [
        "san",
        "encrypt_type",
        "issued_by",
        "des",
        "source",
        "private_key",
        "certificate",
        "issuer_certificate",
        "status",
    ] {
        columns.push(column("cert", name, "character varying", "YES", None));
    }
    for name in ["valid_from", "valid_to"] {
        columns.push(column("cert", name, "bigint", "YES", None));
    }
    columns.push(column("cert", "last_renew_time", "bigint", "NO", None));
    columns.push(column("cert", "is_default", "smallint", "YES", None));
    columns.push(column("cert", "renewal", "smallint", "YES", Some("0")));
    for name in [
        "verify_type",
        "renew_email",
        "ddns_provider",
        "access_key",
        "access_secret",
        "renew_cert_url",
        "last_renew_error",
    ] {
        columns.push(column("cert_renew", name, "character varying", "YES", None));
    }
    columns.push(column(
        "cert_renew",
        "platform",
        "character varying",
        "YES",
        Some("'letsencrypt'::character varying"),
    ));
    columns.push(column(
        "cert_renew",
        "ddns_related_id",
        "bigint",
        "NO",
        Some("0"),
    ));
    columns.push(column(
        "cert_renew",
        "last_renew_time",
        "bigint",
        "YES",
        None,
    ));
    columns.push(column(
        "cert_used_config",
        "service_name",
        "character varying",
        "YES",
        None,
    ));
    for c in &mut columns {
        c.character_maximum_length = match (c.table_name.as_str(), c.column_name.as_str()) {
            ("cert", "certificate" | "issued_by" | "issuer_certificate" | "private_key") => {
                Some(256)
            }
            ("cert", "des" | "domain") => Some(1024),
            ("cert", "san") => Some(1028),
            ("cert", "encrypt_type" | "source" | "status") => Some(16),
            ("cert_renew", "access_key" | "access_secret" | "renew_email") => Some(256),
            ("cert_renew", "ddns_provider" | "platform" | "verify_type") => Some(16),
            ("cert_renew", "last_renew_error" | "renew_cert_url") => Some(1024),
            ("cert_used_config", "service_name") => Some(64),
            _ => None,
        };
    }
    columns.sort_by(|a, b| (&a.table_name, &a.column_name).cmp(&(&b.table_name, &b.column_name)));
    VerifiedSchema { columns }
}

#[cfg(test)]
pub(super) fn extended() -> VerifiedSchema {
    let mut columns = baseline().columns;
    for name in ["platform", "cert_url"] {
        columns.push(column("cert", name, "character varying", "YES", None));
    }
    validate(columns).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supports_only_both_known_layouts_independent_of_order() {
        for expected in [baseline(), extended()] {
            let mut columns = expected.columns.clone();
            columns.reverse();
            assert_eq!(validate(columns).unwrap(), expected);
        }
        assert_eq!(baseline().columns.len(), 37);
        assert_eq!(extended().columns.len(), 39);
    }
    #[test]
    fn rejects_missing_unknown_duplicate_and_partial_extension() {
        let mut columns = baseline().columns;
        columns.pop();
        assert!(
            validate(columns)
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
        let mut columns = baseline().columns;
        columns.push(column("cert", "future", "character varying", "YES", None));
        assert!(
            validate(columns)
                .unwrap_err()
                .to_string()
                .contains("unexpected column")
        );
        for name in ["platform", "cert_url"] {
            let mut columns = baseline().columns;
            columns.push(column("cert", name, "character varying", "YES", None));
            assert!(
                validate(columns)
                    .unwrap_err()
                    .to_string()
                    .contains("missing")
            );
        }
        let mut columns = baseline().columns;
        columns.push(columns[0].clone());
        assert!(validate(columns).is_err());
    }
    #[test]
    fn rejects_attribute_changes_without_disclosing_defaults() {
        for layout in [baseline(), extended()] {
            for index in 0..layout.columns.len() {
                for attribute in [
                    "data_type",
                    "is_nullable",
                    "column_default",
                    "is_identity",
                    "is_generated",
                ] {
                    let mut columns = layout.columns.clone();
                    let c = &mut columns[index];
                    match attribute {
                        "data_type" => c.data_type = "text".into(),
                        "is_nullable" => {
                            c.is_nullable = if c.is_nullable == "YES" { "NO" } else { "YES" }.into()
                        }
                        "column_default" => c.column_default = Some("secret-literal".into()),
                        "is_identity" => c.is_identity = "YES".into(),
                        _ => c.is_generated = "ALWAYS".into(),
                    }
                    let error = validate(columns).unwrap_err().to_string();
                    assert!(error.contains(attribute), "{error}");
                    assert!(!error.contains("secret-literal"));
                }
            }
        }
    }
    #[test]
    fn validates_journal_values_and_character_not_byte_lengths() {
        let schema = baseline();
        let original = super::super::tests::fixture().0.rows.remove(0);
        for (field, value) in [
            ("id", json!(9)),
            ("domain", Value::Null),
            ("valid_to", json!("123")),
            ("is_default", json!(32768)),
            ("source", json!("a".repeat(17))),
            ("source", json!("bad\0value")),
        ] {
            let mut row = original.clone();
            row[field] = value;
            assert!(
                schema
                    .verify_changes(&[RowChange {
                        table: "cert".into(),
                        id: 8,
                        before: None,
                        after: Some(row)
                    }])
                    .is_err(),
                "{field}"
            );
        }
        let mut row = original;
        row["source"] = json!("中".repeat(16));
        schema.verify_row("cert", &row).unwrap();
        row["source"] = json!("中".repeat(17));
        assert!(schema.verify_row("cert", &row).is_err());
    }

    #[test]
    fn captures_length_limits_and_rejects_domain_metadata() {
        let mut columns = baseline().columns;
        let column = columns
            .iter_mut()
            .find(|c| c.table_name == "cert" && c.column_name == "domain")
            .unwrap();
        column.character_maximum_length = Some(128);
        let changed = validate(columns.clone()).unwrap();
        assert_ne!(changed, baseline());
        let column = columns
            .iter_mut()
            .find(|c| c.table_name == "cert" && c.column_name == "domain")
            .unwrap();
        column.domain_name = Some("restricted_domain".into());
        assert!(validate(columns).is_err());
    }

    #[test]
    fn rejects_triggers_invalid_json_and_missing_default_metadata() {
        let columns = serde_json::to_string(&baseline().columns).unwrap();
        for count in ["1", "garbage"] {
            assert!(
                inspect(|sql| Ok(if sql == COLUMNS_SQL {
                    columns.clone()
                } else {
                    count.into()
                }))
                .is_err()
            );
        }
        assert!(inspect(|_| Ok("not JSON".into())).is_err());
        let mut value = serde_json::to_value(baseline().columns).unwrap();
        value[0].as_object_mut().unwrap().remove("column_default");
        assert!(inspect(|_| Ok(value.to_string())).is_err());
    }
}
