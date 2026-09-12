//! Reconciliation and recoverable fnOS certificate mutations. No writes occur while planning.
use super::*;
mod transaction;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Registry {
    #[serde(default)]
    entries: BTreeMap<String, Managed>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Managed {
    #[serde(default = "legacy_digest_version")]
    digest_version: u8,
    paths: Vec<String>,
    fingerprint: String,
    source_ids: Vec<String>,
    domains: Vec<String>,
    target_digest: String,
}
fn legacy_digest_version() -> u8 {
    1
}

#[derive(Clone)]
struct Snapshot {
    rows: Vec<Value>,
    used: Vec<Value>,
    renew: Vec<Value>,
    index: Vec<Value>,
    gateway: Vec<Value>,
    files: BTreeMap<String, Option<String>>,
    registry: Registry,
    registry_bytes: Option<String>,
}
#[derive(Clone)]
struct Action {
    id: String,
    kind: &'static str,
    row: Option<FnosCertRow>,
    local: Option<LocalCandidate>,
    source_ids: Vec<String>,
    public: Value,
}
pub(super) struct Plan {
    snapshot: Snapshot,
    actions: Vec<Action>,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FileChange {
    path: String,
    before: Option<String>,
    after: Option<String>,
    #[serde(default)]
    original_metadata: Option<SavedMetadata>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SavedMetadata {
    mode: u32,
    uid: u32,
    gid: u32,
}
impl SavedMetadata {
    #[cfg(unix)]
    fn read(path: &Path) -> anyhow::Result<Self> {
        validate_fixed_regular_file(path)?;
        let meta = fs::metadata(path)?;
        Ok(Self {
            mode: meta.mode(),
            uid: meta.uid(),
            gid: meta.gid(),
        })
    }
    #[cfg(not(unix))]
    fn read(_path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            mode: 0,
            uid: 0,
            gid: 0,
        })
    }
    #[cfg(unix)]
    fn restore(&self, path: &Path) -> anyhow::Result<()> {
        std::os::unix::fs::chown(path, Some(self.uid), Some(self.gid))?;
        fs::set_permissions(path, fs::Permissions::from_mode(self.mode))?;
        Ok(())
    }
    #[cfg(not(unix))]
    fn restore(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RowChange {
    table: String,
    id: i64,
    before: Option<Value>,
    after: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Journal {
    completed: bool,
    files: Vec<FileChange>,
    rows: Vec<RowChange>,
    registry: FileChange,
}

fn digest(value: &impl Serialize) -> anyhow::Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value)?)))
}
fn registry_path(data_dir: &Path) -> PathBuf {
    data_dir.join("fnos-certificate-sync/registry.json")
}
fn read_optional(path: &Path) -> anyhow::Result<Option<String>> {
    use std::io::Read;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    match options.open(path) {
        Ok(mut file) => {
            if !file.metadata()?.is_file() {
                bail!("Unsafe certificate file")
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(Some(BASE64_STANDARD.encode(bytes)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn table_rows(table: &str) -> anyhow::Result<Vec<Value>> {
    // Table names are exclusively internal constants, never supplied by clients.
    let value = psql(&format!(
        "SELECT coalesce(json_agg(t ORDER BY id),'[]'::json) FROM public.{table} t"
    ))?;
    Ok(serde_json::from_str(value.trim())?)
}
fn file_paths(row: &Value, index: &[Value]) -> BTreeSet<String> {
    let mut paths = ["certificate", "private_key", "issuer_certificate"]
        .into_iter()
        .filter_map(|key| row[key].as_str())
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    for entry in index
        .iter()
        .filter(|entry| entry["certificate"] == row["certificate"])
    {
        if let Some(path) = entry["fullchain"].as_str().filter(|path| !path.is_empty()) {
            paths.insert(path.to_string());
        }
    }
    paths
}
fn raw_row(snapshot: &Snapshot, id: i64) -> anyhow::Result<&Value> {
    snapshot
        .rows
        .iter()
        .find(|row| row["id"].as_i64() == Some(id))
        .ok_or_else(|| anyhow!("fnOS certificate disappeared"))
}
fn target_digest(snapshot: &Snapshot, row: &Value) -> anyhow::Result<String> {
    target_digest_version(snapshot, row, 2)
}
fn target_digest_version(snapshot: &Snapshot, row: &Value, version: u8) -> anyhow::Result<String> {
    let mut row = row.clone();
    if version == 2
        && let Some(fields) = row.as_object_mut()
    {
        fields.remove("is_default");
        fields.remove("updated_time");
    } else if version != 1 && version != 2 {
        bail!("Unsupported certificate registry version")
    }
    let paths = file_paths(&row, &snapshot.index);
    let files = paths
        .iter()
        .map(|path| (path, snapshot.files.get(path)))
        .collect::<Vec<_>>();
    // Binding changes are not content conflicts; they are evaluated separately for deletion.
    let mut entries = snapshot
        .index
        .iter()
        .filter(|e| e["certificate"] == row["certificate"])
        .cloned()
        .collect::<Vec<_>>();
    for entry in &mut entries {
        if let Some(object) = entry.as_object_mut() {
            object.remove("used");
            object.remove("appFlag");
        }
    }
    let renew = snapshot
        .renew
        .iter()
        .filter(|r| r["cert_id"] == row["id"])
        .collect::<Vec<_>>();
    digest(&(row, files, entries, renew))
}
fn verify_schema() -> anyhow::Result<()> {
    let sql = "SELECT table_name,column_name,data_type,is_nullable,column_default FROM information_schema.columns WHERE table_schema='public' AND table_name IN ('cert','cert_used_config','cert_renew') ORDER BY table_name,ordinal_position";
    let output = psql(sql)?;
    // Fingerprint of all 37 columns inspected on the supported fnOS host. Fail closed on upgrades.
    if hex::encode(Sha256::digest(output.as_bytes()))
        != "d87cff3c41c319f3387b5240388c7a6e9fd08d924b255d207e9b2bcfa79b9ed8"
    {
        bail!("Unsupported fnOS certificate database structure")
    }
    let triggers = psql(
        "SELECT count(*) FROM pg_trigger WHERE tgrelid IN ('public.cert'::regclass,'public.cert_renew'::regclass,'public.cert_used_config'::regclass)",
    )?;
    if triggers.trim() != "0" {
        bail!("Unsupported fnOS certificate database triggers")
    }
    Ok(())
}
fn snapshot(data_dir: &Path) -> anyhow::Result<Snapshot> {
    verify_schema()?;
    let rows = table_rows("cert")?;
    let used = table_rows("cert_used_config")?;
    let renew = table_rows("cert_renew")?;
    validate_fixed_regular_file(Path::new(NETWORK_CERT_INDEX))?;
    validate_fixed_regular_file(Path::new(NETWORK_GATEWAY_INDEX))?;
    let index = read_json_array(Path::new(NETWORK_CERT_INDEX))?;
    let gateway = read_json_array(Path::new(NETWORK_GATEWAY_INDEX))?;
    if gateway.iter().any(|entry| {
        ["host", "cert", "key"]
            .iter()
            .any(|key| !entry[key].is_string())
    }) {
        bail!("Unsupported fnOS certificate gateway structure")
    }
    for entry in &index {
        if !entry["san"].is_array()
            || !entry["used"].is_boolean()
            || !entry["appFlag"].is_number()
            || !entry["fullchain"].is_string()
            || !entry["certificate"].is_string()
            || entry["sum"].as_str()
                != Some(path_md5(entry["certificate"].as_str().unwrap_or_default())?.as_str())
        {
            bail!("Unsupported fnOS certificate index structure")
        }
    }
    let mut files = BTreeMap::new();
    for row in &rows {
        for path in file_paths(row, &index) {
            let content = if validate_target_path(Path::new(&path)).is_ok() {
                read_optional(Path::new(&path))?
            } else {
                None
            };
            files.insert(path, content);
        }
    }
    let registry_bytes = read_optional(&registry_path(data_dir))?;
    let registry = match &registry_bytes {
        Some(bytes) => serde_json::from_slice(&BASE64_STANDARD.decode(bytes)?)?,
        None => Registry::default(),
    };
    Ok(Snapshot {
        rows,
        used,
        renew,
        index,
        gateway,
        files,
        registry,
        registry_bytes,
    })
}
fn path_md5(path: &str) -> anyhow::Result<String> {
    let mut child = Command::new("openssl")
        .args(["dgst", "-md5"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("missing digest input"))?
        .write_all(path.as_bytes())?;
    let result = child.wait_with_output()?;
    let output = String::from_utf8(result.stdout)?;
    let hash = output.split_whitespace().last().unwrap_or_default();
    if !result.status.success() || hash.len() != 32 || !hash.bytes().all(|b| b.is_ascii_hexdigit())
    {
        bail!("Unable to calculate fnOS certificate index checksum")
    }
    Ok(hash.to_string())
}
fn references(snapshot: &Snapshot, row: &Value) -> Vec<String> {
    let paths = file_paths(row, &snapshot.index);
    let mut result = BTreeSet::new();
    if row["is_default"].as_i64().unwrap_or(0) != 0 {
        result.insert("default certificate".to_string());
    }
    for binding in snapshot
        .used
        .iter()
        .filter(|binding| binding["cert_id"] == row["id"])
    {
        result.insert(format!(
            "service: {}",
            binding["service_name"].as_str().unwrap_or("unknown")
        ));
    }
    for entry in snapshot
        .index
        .iter()
        .filter(|entry| entry["certificate"] == row["certificate"])
    {
        if entry["used"].as_bool().unwrap_or(true) || entry["appFlag"].as_i64().unwrap_or(-1) != 0 {
            result.insert("fnOS certificate marked in use".to_string());
        }
    }
    for entry in &snapshot.gateway {
        if ["cert", "key"]
            .iter()
            .any(|key| entry[key].as_str().is_some_and(|path| paths.contains(path)))
        {
            result.insert(format!(
                "gateway: {}",
                entry["host"].as_str().unwrap_or("unknown")
            ));
        }
    }
    result.into_iter().collect()
}
fn local_json(local: &LocalCandidate) -> Value {
    json!({"id":local.id,"label":local.label,"valid_from":local.parsed.as_ref().map(|p|p.valid_from),
        "valid_to":local.parsed.as_ref().map(|p|p.valid_to),"fingerprint":local.parsed.as_ref().map(|p|&p.fingerprint)})
}
fn source_ids(candidates: &[LocalCandidate], domains: &[String]) -> Vec<String> {
    candidates
        .iter()
        .filter(|c| c.parsed.as_ref().is_some_and(|p| p.domains == domains))
        .map(|c| c.id.clone())
        .collect()
}
fn actionable(kind: &str) -> bool {
    matches!(kind, "create" | "update" | "adopt" | "delete")
}

pub(super) fn plan(data_dir: &Path, config: &Value) -> anyhow::Result<Plan> {
    if pending_recovery(data_dir)? {
        bail!("Unfinished certificate synchronization requires recovery before planning")
    }
    let snapshot = snapshot(data_dir)?;
    plan_snapshot(snapshot, config)
}
fn plan_snapshot(snapshot: Snapshot, config: &Value) -> anyhow::Result<Plan> {
    // Missing or malformed source collections must never be interpreted as an empty library.
    let source = config
        .pointer("/ssl/certificates")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Local certificate library is unavailable"))?;
    let candidates = local_candidates(config);
    let ids = candidates
        .iter()
        .map(|c| c.id.as_str())
        .collect::<BTreeSet<_>>();
    if source.len() != ids.len() || ids.contains("") {
        bail!("Local certificate library has missing or duplicate identities")
    }
    let version = digest(&(
        &snapshot.rows,
        &snapshot.used,
        &snapshot.renew,
        &snapshot.index,
        &snapshot.gateway,
        &snapshot.files,
        &snapshot.registry,
        source,
    ))?;
    let rows = snapshot
        .rows
        .iter()
        .cloned()
        .map(serde_json::from_value::<FnosCertRow>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut actions = Vec::new();
    let mut represented = BTreeSet::new();
    for row in &rows {
        let id = row.id.to_string();
        let raw = raw_row(&snapshot, row.id)?;
        let managed = snapshot.registry.entries.get(&id);
        let mut compared =
            compare_target(row.clone(), &snapshot.index, &candidates, &snapshot.files);
        let mut kind = "none";
        let mut reason = compared.reason.clone();
        let mut status = compared.status.clone();
        let refs = references(&snapshot, raw);
        let mut related_ids = Vec::new();
        if row.source.as_deref() != Some("system") {
            if let Some(managed) = managed {
                related_ids = managed.source_ids.clone();
                let surviving = candidates
                    .iter()
                    .filter(|c| managed.source_ids.contains(&c.id))
                    .cloned()
                    .collect::<Vec<_>>();
                let same_san = candidates
                    .iter()
                    .filter(|c| {
                        c.parsed
                            .as_ref()
                            .is_some_and(|p| p.domains == managed.domains)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let pool = if same_san.is_empty() {
                    surviving
                } else {
                    same_san
                };
                if target_digest_version(&snapshot, raw, managed.digest_version)?
                    != managed.target_digest
                {
                    status = "conflict".into();
                    reason = Some("fnOS certificate changed outside synchronization".into());
                } else if pool.is_empty() {
                    compared.local = None;
                    if refs.is_empty() {
                        kind = "delete";
                        status = "pending_delete".into();
                        reason = None;
                    } else {
                        status = "delete_blocked".into();
                        reason = Some(refs.join(", "));
                    }
                } else if pool.iter().any(|c| !c.valid && c.parsed.is_none()) {
                    status = "source_invalid".into();
                    reason = Some("Managed source cannot be parsed; deletion is disabled".into());
                } else if let Some(local) =
                    select_best_local(pool.into_iter().filter(|c| c.valid).collect())
                {
                    let parsed = local
                        .parsed
                        .as_ref()
                        .ok_or_else(|| anyhow!("invalid source"))?;
                    related_ids = source_ids(&candidates, &parsed.domains);
                    let collision = rows.iter().any(|other| {
                        other.id != row.id
                            && normalize_domains(split_san(
                                other.san.as_deref().unwrap_or(&other.domain),
                            )) == parsed.domains
                    });
                    if collision {
                        status = "conflict".into();
                        reason = Some("Source SAN conflicts with another fnOS certificate".into());
                    } else if !matches!(compared.status.as_str(), "target_invalid") {
                        let target = compared.target.as_ref();
                        if target.is_some_and(|p| {
                            p.chain_digest == parsed.chain_digest
                                && p.public_key_digest == parsed.public_key_digest
                        }) && managed.source_ids == related_ids
                            && managed.digest_version == 2
                        {
                            status = "up_to_date".into();
                            reason = None;
                        } else {
                            kind = "update";
                            status = "syncable".into();
                            reason = None;
                        }
                    }
                    compared.local = Some(local);
                } else {
                    status = "source_invalid".into();
                    reason =
                        Some("Managed source is invalid or expired; deletion is disabled".into());
                }
            } else if matches!(status.as_str(), "syncable" | "up_to_date") {
                kind = if status == "up_to_date" {
                    "adopt"
                } else {
                    "update"
                };
                if kind == "adopt" {
                    status = "pending_adopt".into();
                }
            }
        }
        if let Some(local) = &compared.local {
            represented.insert(local.id.clone());
            if let Some(parsed) = &local.parsed {
                related_ids = source_ids(&candidates, &parsed.domains);
            }
        }
        // A source mapped to a conflicting target must not also be recreated as a second target.
        if let Some(managed) = managed {
            if matches!(
                status.as_str(),
                "conflict" | "source_invalid" | "target_invalid"
            ) {
                represented.extend(managed.source_ids.iter().cloned());
            } else {
                represented.extend(related_ids.iter().cloned());
            }
        }
        let local = compared.local.clone();
        let mut public = compared_target_json(compared, &BTreeSet::new());
        public["status"] = json!(status);
        public["reason"] = json!(reason);
        public["action_id"] = json!(format!("target:{id}"));
        public["action"] = json!(kind);
        public["managed"] = json!(managed.is_some());
        public["source_ids"] = json!(related_ids);
        public["references"] = json!(refs);
        actions.push(Action {
            id: format!("target:{id}"),
            kind,
            row: Some(row.clone()),
            local,
            source_ids: related_ids,
            public,
        });
    }
    let mut groups: BTreeMap<Vec<String>, Vec<LocalCandidate>> = BTreeMap::new();
    for candidate in &candidates {
        if represented.contains(&candidate.id) {
            continue;
        }
        if let Some(parsed) = &candidate.parsed {
            // Existing invalid/protected targets reserve their SAN rather than causing duplicate creation.
            if rows.iter().any(|row| {
                normalize_domains(split_san(row.san.as_deref().unwrap_or(&row.domain)))
                    == parsed.domains
            }) {
                continue;
            }
            groups
                .entry(parsed.domains.clone())
                .or_default()
                .push(candidate.clone());
        } else {
            actions.push(source_action(
                candidate.clone(),
                vec![candidate.id.clone()],
                "none",
                "source_invalid",
            ));
        }
    }
    for group in groups.into_values() {
        let ids = group.iter().map(|c| c.id.clone()).collect();
        let valid = select_best_local(group.iter().filter(|c| c.valid).cloned().collect());
        if let Some(local) = valid {
            actions.push(source_action(local, ids, "create", "pending_create"));
        } else if let Some(local) = group.into_iter().next() {
            actions.push(source_action(local, ids, "none", "source_invalid"));
        }
    }
    let mut ownership: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, action) in actions.iter().enumerate().filter(|(_, a)| a.row.is_some()) {
        for source_id in &action.source_ids {
            ownership.entry(source_id.clone()).or_default().push(index);
        }
    }
    for owners in ownership.values().filter(|owners| owners.len() > 1) {
        for index in owners {
            let action = &mut actions[*index];
            if action.public["status"] == "protected" {
                continue;
            }
            action.kind = "none";
            action.public["action"] = json!("none");
            action.public["status"] = json!("conflict");
            action.public["reason"] = json!(
                "Source matches multiple fnOS certificates; resolve duplicate ownership first"
            );
        }
    }
    Ok(Plan {
        snapshot,
        actions,
        version,
    })
}
fn source_action(
    local: LocalCandidate,
    ids: Vec<String>,
    kind: &'static str,
    status: &str,
) -> Action {
    let id = format!("source:{}", local.id);
    let public = json!({"target_id":"","action_id":id,"action":kind,"managed":false,"source_ids":ids,"references":[],
        "domain":local.parsed.as_ref().and_then(|p|p.domains.first()).cloned().unwrap_or_else(||local.label.clone()),
        "san":local.parsed.as_ref().map(|p|p.domains.clone()).unwrap_or_default(),"source":"local","renewal":false,
        "valid_from":null,"valid_to":null,"fingerprint":null,"status":status,"reason":null,"local":local_json(&local)});
    Action {
        id,
        kind,
        row: None,
        local: Some(local),
        source_ids: ids,
        public,
    }
}
impl Plan {
    pub fn details(&self) -> (Value, Vec<Value>) {
        let items = self
            .actions
            .iter()
            .map(|a| a.public.clone())
            .collect::<Vec<_>>();
        let count = |kind| self.actions.iter().filter(|a| a.kind == kind).count();
        (
            json!({"total":items.len(),"syncable":self.actions.iter().filter(|a|actionable(a.kind)).count(),
            "up_to_date":items.iter().filter(|i|i["status"]=="up_to_date").count(),
            "create":count("create"),"update":count("update"),"delete":count("delete"),"adopt":count("adopt")}),
            items,
        )
    }
}

fn json_sql(value: &Value) -> String {
    format!("{}::jsonb", sql_text_expression(&value.to_string()))
}
fn apply_rows(changes: &[RowChange], reverse: bool) -> anyhow::Result<()> {
    let mut sql = String::from(
        "BEGIN; SET LOCAL lock_timeout='5s'; LOCK TABLE public.cert, public.cert_renew, public.cert_used_config IN SHARE ROW EXCLUSIVE MODE;\n",
    );
    for cert in changes.iter().filter(|c| c.table == "cert") {
        let renewals = |after: bool| {
            changes
                .iter()
                .filter(|c| c.table == "cert_renew")
                .filter_map(|c| {
                    if after {
                        c.after.as_ref()
                    } else {
                        c.before.as_ref()
                    }
                })
                .filter(|r| r["cert_id"].as_i64() == Some(cert.id))
                .cloned()
                .collect::<Vec<_>>()
        };
        let before = json_sql(&json!(renewals(false)));
        let after = json_sql(&json!(renewals(true)));
        let current = format!(
            "(SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY id),'[]'::jsonb) FROM public.cert_renew r WHERE cert_id={})",
            cert.id
        );
        let condition = if reverse {
            format!("{current} = {before} OR {current} = {after}")
        } else {
            format!("{current} = {before}")
        };
        sql.push_str(&format!("SELECT CASE WHEN {condition} THEN 1 ELSE (random()::text || 'renewal changed')::integer END;\n"));
    }
    for change in changes {
        if !matches!(change.table.as_str(), "cert" | "cert_renew") {
            bail!("Invalid recovery table")
        }
        let (before, after) = if reverse {
            (&change.after, &change.before)
        } else {
            (&change.before, &change.after)
        };
        // Retrying a partially completed recovery accepts only the original or our applied row.
        let current = format!(
            "(SELECT to_jsonb(t) FROM public.{} t WHERE id={})",
            change.table, change.id
        );
        let before_expr = before
            .as_ref()
            .map(json_sql)
            .unwrap_or_else(|| "NULL::jsonb".into());
        let after_expr = after
            .as_ref()
            .map(json_sql)
            .unwrap_or_else(|| "NULL::jsonb".into());
        sql.push_str(&format!("SELECT CASE WHEN {current} IS NOT DISTINCT FROM {before_expr} {} THEN 1 ELSE (random()::text || 'concurrent certificate change')::integer END;\n",
            if reverse { format!("OR {current} IS NOT DISTINCT FROM {after_expr}") } else { String::new() }));
        if change.table == "cert" && after.is_none() {
            sql.push_str(&format!("SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM public.cert_used_config WHERE cert_id={}) THEN 1 ELSE (random()::text || 'certificate in use')::integer END;\n", change.id));
        }
        sql.push_str(&format!(
            "DELETE FROM public.{} WHERE id={};\n",
            change.table, change.id
        ));
        if let Some(row) = after {
            sql.push_str(&format!(
                "INSERT INTO public.{} SELECT * FROM jsonb_populate_record(NULL::public.{},{});\n",
                change.table,
                change.table,
                json_sql(row)
            ));
        }
    }
    sql.push_str("COMMIT;");
    // PostgreSQL error DETAIL can contain row contents (including renewal credentials).
    psql(&sql).map_err(|_| {
        anyhow!("fnOS certificate database transaction failed or changed concurrently")
    })?;
    Ok(())
}
fn ensure_parent(path: &Path, data_dir: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Missing file parent"))?;
    if path == Path::new(NETWORK_CERT_INDEX) {
        return validate_fixed_regular_file(path);
    }
    let root = if path.starts_with(CERT_ROOT) {
        PathBuf::from(CERT_ROOT)
    } else if path.starts_with(data_dir.join("fnos-certificate-sync")) {
        data_dir.join("fnos-certificate-sync")
    } else {
        bail!("Unsafe certificate write path")
    };
    fs::create_dir_all(&root)?;
    if fs::symlink_metadata(&root)?.file_type().is_symlink() {
        bail!("Unsafe certificate directory")
    }
    if root != Path::new(CERT_ROOT) {
        set_private_directory_permissions(&root)?;
        if let Some(parent) = root.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
    }
    let relative = parent.strip_prefix(&root)?;
    let mut current = root.clone();
    for part in relative.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            bail!("Unsafe certificate directory")
        }
        current.push(part);
        if !current.exists() {
            fs::create_dir(&current)?;
            set_private_directory_permissions(&current)?;
            if let Some(parent) = current.parent() {
                fs::File::open(parent)?.sync_all()?;
            }
        }
        if fs::symlink_metadata(&current)?.file_type().is_symlink() {
            bail!("Unsafe certificate directory")
        }
    }
    if !fs::canonicalize(parent)?.starts_with(fs::canonicalize(root)?) {
        bail!("Unsafe certificate directory")
    }
    Ok(())
}
fn write_content(path: &Path, bytes: &[u8], data_dir: &Path) -> anyhow::Result<()> {
    write_content_with_metadata(path, bytes, data_dir, None)
}
fn write_content_with_metadata(
    path: &Path,
    bytes: &[u8],
    data_dir: &Path,
    saved: Option<&SavedMetadata>,
) -> anyhow::Result<()> {
    ensure_parent(path, data_dir)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                bail!("Unsafe certificate file")
            }
            Some(metadata)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let parent = path.parent().ok_or_else(|| anyhow!("Missing parent"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    if let Some(saved) = saved {
        saved.restore(temp.path())?;
    } else if let Some(metadata) = metadata {
        preserve_file_metadata(temp.path(), &metadata)?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn check_file(change: &FileChange, reverse: bool) -> anyhow::Result<()> {
    let current = read_optional(Path::new(&change.path))?;
    if current.is_some()
        && let Some(saved) = &change.original_metadata
        && SavedMetadata::read(Path::new(&change.path))? != *saved
    {
        bail!("Certificate file permissions changed externally; recovery backup preserved")
    }
    if current != change.before && !(reverse && current == change.after) {
        bail!("Certificate files changed externally; recovery backup preserved")
    }
    Ok(())
}
fn apply_file(change: &FileChange, reverse: bool, data_dir: &Path) -> anyhow::Result<()> {
    let expected = if reverse {
        &change.before
    } else {
        &change.after
    };
    ensure_parent(Path::new(&change.path), data_dir)?;
    check_file(change, true)?;
    if read_optional(Path::new(&change.path))? == *expected {
        return Ok(());
    }
    check_file(change, reverse)?;
    match expected {
        Some(value) => {
            if reverse
                && change.before.is_some()
                && change.original_metadata.is_none()
                && read_optional(Path::new(&change.path))?.is_none()
            {
                bail!("Recovery log lacks original file permissions; manual recovery required")
            }
            write_content_with_metadata(
                Path::new(&change.path),
                &BASE64_STANDARD.decode(value)?,
                data_dir,
                if reverse {
                    change.original_metadata.as_ref()
                } else {
                    None
                },
            )
        }
        None => {
            ensure_parent(Path::new(&change.path), data_dir)?;
            validate_fixed_regular_file(Path::new(&change.path))?;
            fs::remove_file(&change.path)?;
            if let Some(parent) = Path::new(&change.path).parent() {
                fs::File::open(parent)?.sync_all()?;
            }
            Ok(())
        }
    }
}
fn save_journal(path: &Path, journal: &Journal, data_dir: &Path) -> anyhow::Result<()> {
    write_content(path, &serde_json::to_vec(journal)?, data_dir)
}
fn rollback(journal: &Journal, data_dir: &Path) -> anyhow::Result<()> {
    transaction::restore(
        journal,
        &mut transaction::Native {
            data_dir,
            selected: &[],
        },
    )
}

fn process_lock(data_dir: &Path) -> anyhow::Result<fs::File> {
    let path = data_dir.join("fnos-certificate-sync/execution.lock");
    ensure_parent(&path, data_dir)?;
    if path.exists() {
        validate_fixed_regular_file(&path)?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock()
        .map_err(|_| anyhow!("Another fnOS certificate synchronization is running"))?;
    Ok(file)
}
pub(super) fn recover(data_dir: &Path) -> anyhow::Result<()> {
    let _lock = process_lock(data_dir)?;
    recover_locked(data_dir)
}
fn recover_locked(data_dir: &Path) -> anyhow::Result<()> {
    let root = data_dir.join("fnos-certificate-sync/transactions");
    if !root.exists() {
        return Ok(());
    }
    let mut paths = fs::read_dir(&root)?.collect::<Result<Vec<_>, _>>()?;
    paths.sort_by_key(|p| p.file_name());
    for path in paths {
        if path.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let mut journal: Journal = serde_json::from_slice(&fs::read(path.path())?)?;
        if journal.completed {
            continue;
        }
        verify_schema()?;
        rollback(&journal, data_dir)
            .context("Unfinished certificate synchronization needs recovery; backup preserved")?;
        journal.completed = true;
        save_journal(&path.path(), &journal, data_dir)?;
    }
    Ok(())
}
fn pending_recovery(data_dir: &Path) -> anyhow::Result<bool> {
    let root = data_dir.join("fnos-certificate-sync/transactions");
    if !root.exists() {
        return Ok(false);
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let journal: Journal = serde_json::from_slice(&fs::read(path)?)?;
            if !journal.completed {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn chain_parts(pem: &str) -> anyhow::Result<(String, String)> {
    parse_certificate(pem)?;
    let end = pem
        .find("-----END CERTIFICATE-----")
        .ok_or_else(|| anyhow!("Missing leaf certificate"))?
        + "-----END CERTIFICATE-----".len();
    Ok((
        format!("{}\n", pem[..end].trim()),
        format!("{}\n", pem[end..].trim()),
    ))
}
fn add_file(
    files: &mut BTreeMap<String, FileChange>,
    snapshot: &Snapshot,
    path: &str,
    after: Option<String>,
) -> anyhow::Result<()> {
    if path.is_empty() {
        bail!("Missing certificate path")
    }
    let change = FileChange {
        path: path.to_string(),
        before: snapshot.files.get(path).cloned().flatten(),
        after,
        original_metadata: if snapshot.files.get(path).is_some_and(Option::is_some) {
            Some(SavedMetadata::read(Path::new(path))?)
        } else {
            None
        },
    };
    if let Some(existing) = files.get(path)
        && existing.after != change.after
    {
        bail!("Shared certificate file has conflicting changes")
    }
    files.insert(path.to_string(), change);
    Ok(())
}
fn set_certificate_files(
    files: &mut BTreeMap<String, FileChange>,
    snapshot: &Snapshot,
    row: &Value,
    index: &Value,
    local: &LocalCandidate,
) -> anyhow::Result<()> {
    let (leaf, issuer) = chain_parts(&local.cert)?;
    let cert = row["certificate"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing certificate path"))?;
    let key = row["private_key"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing key path"))?;
    let full = index["fullchain"].as_str().unwrap_or_default();
    add_file(
        files,
        snapshot,
        cert,
        Some(BASE64_STANDARD.encode(if full.is_empty() { &local.cert } else { &leaf })),
    )?;
    add_file(
        files,
        snapshot,
        key,
        Some(BASE64_STANDARD.encode(&local.key)),
    )?;
    if !full.is_empty() {
        add_file(
            files,
            snapshot,
            full,
            Some(BASE64_STANDARD.encode(&local.cert)),
        )?;
    }
    if let Some(path) = row["issuer_certificate"].as_str().filter(|p| !p.is_empty()) {
        add_file(files, snapshot, path, Some(BASE64_STANDARD.encode(issuer)))?;
    }
    Ok(())
}
fn update_domains(row: &mut Value, entry: &mut Value, parsed: &ParsedCertificate) {
    let old = row["san"]
        .as_str()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| row["domain"].as_str().unwrap_or_default());
    if normalize_domains(split_san(old)) != parsed.domains {
        row["domain"] = json!(parsed.domains[0]);
        entry["domain"] = json!(parsed.domains[0]);
    }
    row["san"] = json!(parsed.domains.join(","));
    entry["san"] = json!(parsed.domains);
}

fn prepare(plan: &Plan, selected: &[&Action], data_dir: &Path) -> anyhow::Result<Journal> {
    let snapshot = &plan.snapshot;
    let mut index = snapshot.index.clone();
    let mut rows = Vec::new();
    let mut files = BTreeMap::new();
    let mut registry = snapshot.registry.clone();
    let mut simulated = snapshot.clone();
    let now = time_utils::now_ms();
    let mut managed_updates = Vec::new();
    for action in selected {
        let id = match &action.row {
            Some(row) => row.id,
            None => psql("SELECT nextval('public.cert_id_seq')")?
                .trim()
                .parse::<i64>()?,
        };
        let original = snapshot
            .rows
            .iter()
            .find(|row| row["id"].as_i64() == Some(id))
            .cloned();
        if action.kind == "delete" {
            let original = original.ok_or_else(|| anyhow!("Missing deletion target"))?;
            if !references(snapshot, &original).is_empty() {
                bail!("Certificate is still in use")
            }
            let paths = file_paths(&original, &snapshot.index);
            index.retain(|entry| entry["certificate"] != original["certificate"]);
            for path in paths {
                let shared = snapshot
                    .rows
                    .iter()
                    .filter(|row| row["id"] != original["id"])
                    .any(|row| file_paths(row, &snapshot.index).contains(&path));
                if !shared {
                    add_file(&mut files, snapshot, &path, None)?;
                }
            }
            rows.push(RowChange {
                table: "cert".into(),
                id,
                before: Some(original),
                after: None,
            });
            registry.entries.remove(&id.to_string());
        } else {
            let local = action
                .local
                .as_ref()
                .ok_or_else(|| anyhow!("Missing certificate source"))?;
            let parsed = local
                .parsed
                .as_ref()
                .ok_or_else(|| anyhow!("Invalid certificate source"))?;
            let mut row = original.clone().unwrap_or_else(||json!({"id":id,"domain":parsed.domains[0],"san":parsed.domains.join(","),
                "valid_from":parsed.valid_from,"valid_to":parsed.valid_to,"encrypt_type":parsed.encrypt_type,"issued_by":parsed.issued_by,
                "last_renew_time":now,"des":"fn-knock certificate sync","is_default":0,"renewal":0,"source":"upload",
                "private_key":"","certificate":"","issuer_certificate":"","status":"suc","created_time":now,"updated_time":now}));
            if action.kind == "create" {
                let dir = Path::new(CERT_ROOT).join(format!("fn-knock-{}", Uuid::new_v4()));
                row["certificate"] = json!(dir.join("cert.crt").to_string_lossy());
                row["private_key"] = json!(dir.join("private.key").to_string_lossy());
                row["issuer_certificate"] = json!(dir.join("issued.crt").to_string_lossy());
                index.push(json!({"domain":parsed.domains[0],"san":parsed.domains,
                    "certificate":row["certificate"],"privateKey":row["private_key"],"fullchain":dir.join("fullchain.crt").to_string_lossy(),
                    "validFrom":parsed.valid_from,"validTo":parsed.valid_to,"sum":path_md5(row["certificate"].as_str().unwrap_or_default())?,"used":false,"appFlag":0}));
            }
            let entry = index
                .iter_mut()
                .find(|entry| entry["certificate"] == row["certificate"])
                .ok_or_else(|| anyhow!("Missing fnOS index entry"))?;
            if action.kind != "adopt" {
                update_domains(&mut row, entry, parsed);

                row["valid_from"] = json!(parsed.valid_from);
                row["valid_to"] = json!(parsed.valid_to);
                row["encrypt_type"] = json!(parsed.encrypt_type);
                row["issued_by"] = json!(parsed.issued_by);
                row["status"] = json!("suc");
                row["last_renew_time"] = json!(now);

                entry["validFrom"] = json!(parsed.valid_from);
                entry["validTo"] = json!(parsed.valid_to);
                set_certificate_files(&mut files, snapshot, &row, entry, local)?;
            }
            row["renewal"] = json!(0);
            row["updated_time"] = json!(now);
            rows.push(RowChange {
                table: "cert".into(),
                id,
                before: original,
                after: Some(row),
            });
            managed_updates.push((
                id,
                action.source_ids.clone(),
                parsed.domains.clone(),
                parsed.fingerprint.clone(),
            ));
        }
        for renewal in snapshot
            .renew
            .iter()
            .filter(|r| r["cert_id"].as_i64() == Some(id))
        {
            rows.push(RowChange {
                table: "cert_renew".into(),
                id: renewal["id"]
                    .as_i64()
                    .ok_or_else(|| anyhow!("Invalid renewal identity"))?,
                before: Some(renewal.clone()),
                after: None,
            });
        }
    }
    // Never overwrite a shared file used by a certificate outside this transaction.
    let selected_ids = selected
        .iter()
        .filter_map(|a| a.row.as_ref().map(|r| r.id))
        .collect::<BTreeSet<_>>();
    for change in files.values().filter(|c| c.before != c.after) {
        if snapshot
            .rows
            .iter()
            .filter(|r| !selected_ids.contains(&r["id"].as_i64().unwrap_or_default()))
            .any(|r| file_paths(r, &snapshot.index).contains(&change.path))
        {
            bail!("Certificate file is shared with an unselected target")
        }
    }
    for change in &rows {
        let table = if change.table == "cert" {
            &mut simulated.rows
        } else {
            &mut simulated.renew
        };
        table.retain(|row| row["id"].as_i64() != Some(change.id));
        if let Some(row) = &change.after {
            table.push(row.clone());
        }
    }
    simulated.index = index.clone();
    for change in files.values() {
        simulated
            .files
            .insert(change.path.clone(), change.after.clone());
    }
    for (id, source_ids, domains, fingerprint) in managed_updates {
        let target_digest = target_digest(&simulated, raw_row(&simulated, id)?)?;
        registry.entries.insert(
            id.to_string(),
            Managed {
                digest_version: 2,
                paths: file_paths(raw_row(&simulated, id)?, &simulated.index)
                    .into_iter()
                    .collect(),
                fingerprint,
                source_ids,
                domains,
                target_digest,
            },
        );
    }
    let index_bytes = read_optional(Path::new(NETWORK_CERT_INDEX))?;
    files.insert(
        NETWORK_CERT_INDEX.into(),
        FileChange {
            path: NETWORK_CERT_INDEX.into(),
            before: index_bytes,
            original_metadata: Some(SavedMetadata::read(Path::new(NETWORK_CERT_INDEX))?),
            after: Some(BASE64_STANDARD.encode(serde_json::to_vec(&index)?)),
        },
    );
    Ok(Journal {
        completed: false,
        files: files.into_values().collect(),
        rows,
        registry: FileChange {
            path: registry_path(data_dir).to_string_lossy().into(),
            before: snapshot.registry_bytes.clone(),
            original_metadata: if snapshot.registry_bytes.is_some() {
                Some(SavedMetadata::read(&registry_path(data_dir))?)
            } else {
                None
            },
            after: Some(BASE64_STANDARD.encode(serde_json::to_vec(&registry)?)),
        },
    })
}

pub(super) fn execute(
    data_dir: &Path,
    config: &Value,
    action_ids: Option<&[String]>,
    version: Option<&str>,
    legacy_ids: Option<&[i64]>,
) -> Result<SyncSummary, SyncExecutionError> {
    let mut attempted = action_ids.unwrap_or_default().to_vec();
    let mut work = || -> anyhow::Result<SyncSummary> {
        let _lock = process_lock(data_dir)?;
        recover_locked(data_dir)?;
        let plan = plan(data_dir, config)?;
        if version.is_some_and(|v| v != plan.version) {
            bail!("stale certificate synchronization plan")
        }
        let selected = plan
            .actions
            .iter()
            .filter(|action| {
                if let Some(ids) = legacy_ids {
                    return action.kind == "update"
                        && action
                            .row
                            .as_ref()
                            .is_some_and(|r| ids.is_empty() || ids.contains(&r.id));
                }
                actionable(action.kind) && action_ids.is_none_or(|ids| ids.contains(&action.id))
            })
            .collect::<Vec<_>>();
        if let Some(ids) = action_ids
            && (ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                || ids.iter().any(|id| !selected.iter().any(|a| &a.id == id)))
        {
            bail!("stale certificate synchronization plan: action unavailable")
        }
        let mut summary = SyncSummary {
            synced: 0,
            skipped: legacy_ids.map_or(0, |ids| ids.len().saturating_sub(selected.len())),
            failed: 0,
            rolled_back: false,
            created: 0,
            updated: 0,
            deleted: 0,
            adopted: 0,
        };
        if selected.is_empty() {
            return Ok(summary);
        }
        attempted = selected.iter().map(|action| action.id.clone()).collect();
        let journal = prepare(&plan, &selected, data_dir)?;
        // IDs may have been reserved, but no certificate changes have happened at this point.
        if self::plan(data_dir, config)?.version != plan.version {
            bail!("stale certificate synchronization plan")
        }
        let path = data_dir
            .join("fnos-certificate-sync/transactions")
            .join(format!("{}-{}.json", time_utils::now_ms(), Uuid::new_v4()));
        save_journal(&path, &journal, data_dir)?;
        let result = transaction::apply(
            &journal,
            &mut transaction::Native {
                data_dir,
                selected: &selected,
            },
        );
        let mut journal = journal;
        if let Err(error) = result {
            match rollback(&journal, data_dir) {
                Ok(()) => {
                    journal.completed = true;
                    save_journal(&path, &journal, data_dir)?;
                    bail!("{error}; fnOS changes were rolled back")
                }
                Err(_) => bail!(
                    "{error}; rollback stopped; recovery backup preserved at {}",
                    path.display()
                ),
            }
        }
        journal.completed = true;
        save_journal(&path, &journal, data_dir)?;
        for file in journal.files.iter().filter(|file| file.after.is_none()) {
            if let Some(parent) = Path::new(&file.path).parent()
                && parent.parent() == Some(Path::new(CERT_ROOT))
                && parent
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("fn-knock-"))
            {
                let _ = fs::remove_dir(parent); // Only removes an empty, exclusively generated directory.
            }
        }
        for action in selected {
            summary.synced += 1;
            match action.kind {
                "create" => summary.created += 1,
                "update" => summary.updated += 1,
                "delete" => summary.deleted += 1,
                "adopt" => summary.adopted += 1,
                _ => {}
            }
        }
        if let Err(error) = prune_transactions(data_dir) {
            tracing::warn!(%error, "failed to prune completed certificate transactions");
        }
        Ok(summary)
    };
    let result = work();
    result.map_err(|error| SyncExecutionError::new(error, &attempted))
}
fn verify_applied(journal: &Journal, selected: &[&Action], data_dir: &Path) -> anyhow::Result<()> {
    let snapshot = snapshot(data_dir)?;
    for change in &journal.rows {
        let table = if change.table == "cert" {
            &snapshot.rows
        } else {
            &snapshot.renew
        };
        if table.iter().find(|r| r["id"].as_i64() == Some(change.id)) != change.after.as_ref() {
            bail!("fnOS certificate database verification failed")
        }
    }
    for file in journal
        .files
        .iter()
        .filter(|f| f.path != NETWORK_CERT_INDEX)
    {
        if read_optional(Path::new(&file.path))? != file.after {
            bail!("fnOS certificate file verification failed")
        }
    }
    let expected_index: Vec<Value> = serde_json::from_slice(
        &BASE64_STANDARD.decode(
            journal
                .files
                .iter()
                .find(|f| f.path == NETWORK_CERT_INDEX)
                .and_then(|f| f.after.as_ref())
                .ok_or_else(|| anyhow!("Missing index recovery record"))?,
        )?,
    )?;
    if snapshot.index != expected_index {
        bail!("fnOS certificate index verification failed")
    }
    for change in journal.rows.iter().filter(|c| c.table == "cert") {
        if change.after.is_none() {
            if let Some(row) = &change.before
                && (snapshot
                    .index
                    .iter()
                    .any(|e| e["certificate"] == row["certificate"])
                    || !references(&snapshot, row).is_empty())
            {
                bail!("Deleted certificate remains referenced")
            }
        } else if let Some(row) = &change.after {
            let paths = file_paths(row, &snapshot.index);
            let local = selected
                .iter()
                .filter_map(|a| a.local.as_ref())
                .find(|c| {
                    c.parsed.as_ref().is_some_and(|p| {
                        p.domains
                            == normalize_domains(split_san(row["san"].as_str().unwrap_or_default()))
                    })
                })
                .ok_or_else(|| anyhow!("Missing expected source"))?;
            for mapping in &snapshot.gateway {
                if mapping["cert"].as_str().is_some_and(|p| paths.contains(p)) {
                    let host = mapping["host"]
                        .as_str()
                        .ok_or_else(|| anyhow!("Invalid gateway mapping"))?;
                    verify_host(
                        host,
                        local
                            .parsed
                            .as_ref()
                            .ok_or_else(|| anyhow!("Invalid source"))?,
                    )?;
                }
            }
        }
    }
    Ok(())
}
fn verify_host(host: &str, expected: &ParsedCertificate) -> anyhow::Result<()> {
    let mut args = vec!["8", "openssl", "s_client", "-connect", "127.0.0.1:443"];
    if host != "fallback" {
        args.extend(["-servername", host]);
    }
    let mut child = Command::new("timeout")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Missing TLS probe input"))?
        .write_all(b"Q\n")?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!("fnOS TLS probe failed for {host}")
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let start = text
        .find("-----BEGIN CERTIFICATE-----")
        .ok_or_else(|| anyhow!("TLS probe returned no certificate"))?;
    let end = start
        + text[start..]
            .find("-----END CERTIFICATE-----")
            .ok_or_else(|| anyhow!("Incomplete TLS certificate"))?
        + "-----END CERTIFICATE-----".len();
    if parse_certificate(&text[start..end])?.fingerprint != expected.fingerprint {
        bail!("fnOS TLS fingerprint mismatch for {host}")
    }
    Ok(())
}
fn prune_transactions(data_dir: &Path) -> anyhow::Result<()> {
    let mut completed = Vec::new();
    for entry in fs::read_dir(data_dir.join("fnos-certificate-sync/transactions"))? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let journal: Journal = serde_json::from_slice(&fs::read(entry.path())?)?;
        if journal.completed {
            completed.push(entry.path());
        }
    }
    completed.sort();
    let remove = completed.len().saturating_sub(BACKUP_KEEP_COUNT);
    for path in completed.into_iter().take(remove) {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::generate_simple_self_signed;

    fn fixture() -> (Snapshot, Value) {
        let cert = generate_simple_self_signed(vec!["sync.example.test".into()]).unwrap();
        let pem = cert.cert.pem();
        let key = cert.signing_key.serialize_pem();
        let parsed = parse_certificate(&pem).unwrap();
        let row = json!({"id":8,"domain":"sync.example.test","san":"sync.example.test","valid_from":parsed.valid_from,
            "valid_to":parsed.valid_to,"encrypt_type":parsed.encrypt_type,"issued_by":parsed.issued_by,"last_renew_time":1,
            "des":"","is_default":0,"renewal":0,"source":"upload","private_key":"/key","certificate":"/cert",
            "issuer_certificate":"","status":"suc","created_time":1,"updated_time":1});
        let index = json!({"certificate":"/cert","privateKey":"/key","fullchain":"","domain":"sync.example.test",
            "san":["sync.example.test"],"validFrom":parsed.valid_from,"validTo":parsed.valid_to,"sum":"fixture","used":false,"appFlag":0});
        let snapshot = Snapshot {
            rows: vec![row],
            used: vec![],
            renew: vec![],
            index: vec![index],
            gateway: vec![],
            files: BTreeMap::from([
                ("/cert".into(), Some(BASE64_STANDARD.encode(&pem))),
                ("/key".into(), Some(BASE64_STANDARD.encode(&key))),
            ]),
            registry: Registry::default(),
            registry_bytes: None,
        };
        let config = json!({"ssl":{"certificates":[{"id":"local-1","label":"test","cert":pem,"key":key,"updated_at":"1"}]}});
        (snapshot, config)
    }
    fn managed(snapshot: &mut Snapshot) {
        snapshot.registry.entries.insert(
            "8".into(),
            Managed {
                digest_version: 2,
                paths: vec!["/cert".into(), "/key".into()],
                fingerprint: String::new(),
                source_ids: vec!["local-1".into()],
                domains: vec!["sync.example.test".into()],
                target_digest: target_digest(snapshot, &snapshot.rows[0]).unwrap(),
            },
        );
    }
    fn status(snapshot: Snapshot, config: &Value) -> String {
        plan_snapshot(snapshot, config).unwrap().actions[0].public["status"]
            .as_str()
            .unwrap()
            .into()
    }

    #[test]
    fn matching_certificates_are_adopted_only_when_executed() {
        let (snapshot, config) = fixture();
        let plan = plan_snapshot(snapshot, &config).unwrap();
        assert_eq!(plan.actions[0].kind, "adopt");
        assert!(plan.snapshot.registry.entries.is_empty());
    }
    #[test]
    fn creates_only_one_certificate_per_san_group() {
        let (mut snapshot, mut config) = fixture();
        snapshot.rows.clear();
        snapshot.index.clear();
        snapshot.files.clear();
        let mut duplicate = config["ssl"]["certificates"][0].clone();
        duplicate["id"] = json!("local-2");
        config["ssl"]["certificates"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        let plan = plan_snapshot(snapshot, &config).unwrap();
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].kind, "create");
        assert_eq!(plan.actions[0].source_ids.len(), 2);
    }
    #[test]
    fn only_managed_removed_sources_are_deleted() {
        let (mut snapshot, _) = fixture();
        let empty = json!({"ssl":{"certificates":[]}});
        assert_eq!(status(snapshot.clone(), &empty), "unmatched");
        managed(&mut snapshot);
        assert_eq!(status(snapshot, &empty), "pending_delete");
    }
    #[test]
    fn missing_library_and_invalid_sources_never_delete() {
        let (mut snapshot, mut config) = fixture();
        managed(&mut snapshot);
        assert!(plan_snapshot(snapshot.clone(), &json!({})).is_err());
        config["ssl"]["certificates"][0]["cert"] = json!("invalid");
        assert_eq!(status(snapshot, &config), "source_invalid");
    }
    #[test]
    fn external_target_edits_conflict_and_system_certificates_are_protected() {
        let (mut snapshot, config) = fixture();
        managed(&mut snapshot);
        snapshot.rows[0]["des"] = json!("external modification");
        assert_eq!(status(snapshot.clone(), &config), "conflict");
        snapshot.rows[0]["source"] = json!("system");
        assert_eq!(status(snapshot, &config), "protected");
    }
    #[test]
    fn all_usage_sources_block_deletion() {
        let (snapshot, _) = fixture();
        let config = json!({"ssl":{"certificates":[]}});
        for source in 0..5 {
            let mut s = snapshot.clone();
            match source {
                0 => s.used.push(json!({"cert_id":8,"service_name":"web"})),
                1 => s.rows[0]["is_default"] = json!(1),
                2 => s.index[0]["used"] = json!(true),
                3 => s.index[0]["appFlag"] = json!(2),
                _ => s
                    .gateway
                    .push(json!({"host":"sync.example.test","cert":"/cert","key":"/key"})),
            }
            managed(&mut s);
            assert_eq!(status(s, &config), "delete_blocked");
        }
    }
    #[test]
    fn remaining_same_san_source_prevents_deletion() {
        let (mut snapshot, mut config) = fixture();
        managed(&mut snapshot);
        config["ssl"]["certificates"][0]["id"] = json!("replacement");
        let plan = plan_snapshot(snapshot, &config).unwrap();
        assert_eq!(plan.actions[0].kind, "update");
        assert_eq!(plan.actions[0].source_ids, vec!["replacement"]);
    }
    #[test]
    fn san_changes_keep_the_mapped_target_identity() {
        let (mut snapshot, mut config) = fixture();
        managed(&mut snapshot);
        let cert = generate_simple_self_signed(vec!["changed.example.test".into()]).unwrap();
        config["ssl"]["certificates"][0]["cert"] = json!(cert.cert.pem());
        config["ssl"]["certificates"][0]["key"] = json!(cert.signing_key.serialize_pem());
        let plan = plan_snapshot(snapshot, &config).unwrap();
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].kind, "update");
        assert_eq!(plan.actions[0].row.as_ref().unwrap().id, 8);
    }
    #[test]
    fn fullchain_and_leaf_must_agree() {
        let (mut snapshot, config) = fixture();
        snapshot.index[0]["fullchain"] = json!("/fullchain");
        snapshot
            .files
            .insert("/fullchain".into(), snapshot.files["/cert"].clone());
        assert_eq!(status(snapshot.clone(), &config), "pending_adopt");
        let other = generate_simple_self_signed(vec!["other.example.test".into()]).unwrap();
        snapshot.files.insert(
            "/fullchain".into(),
            Some(BASE64_STANDARD.encode(other.cert.pem())),
        );
        assert_eq!(status(snapshot, &config), "target_invalid");
    }
    #[test]
    fn snapshot_versions_track_sources_files_and_usage() {
        let (snapshot, mut config) = fixture();
        let version = plan_snapshot(snapshot.clone(), &config).unwrap().version;
        config["ssl"]["certificates"][0]["updated_at"] = json!("2");
        assert_ne!(
            version,
            plan_snapshot(snapshot.clone(), &config).unwrap().version
        );
        let mut changed = snapshot.clone();
        changed.gateway.push(json!({"host":"new"}));
        assert_ne!(
            plan_snapshot(snapshot, &config).unwrap().version,
            plan_snapshot(changed, &config).unwrap().version
        );
    }
    #[test]
    fn file_recovery_is_idempotent_and_rejects_external_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fnos-certificate-sync/test");
        let change = FileChange {
            path: path.to_string_lossy().into(),
            before: None,
            after: Some(BASE64_STANDARD.encode("applied")),
            original_metadata: None,
        };
        apply_file(&change, false, dir.path()).unwrap();
        apply_file(&change, false, dir.path()).unwrap();
        fs::write(&path, "external").unwrap();
        assert!(apply_file(&change, true, dir.path()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");
        fs::write(&path, "applied").unwrap();
        apply_file(&change, true, dir.path()).unwrap();
        apply_file(&change, true, dir.path()).unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn expired_sources_do_not_delete_and_duplicate_targets_conflict() {
        let (mut snapshot, mut config) = fixture();
        managed(&mut snapshot);
        let mut duplicate = snapshot.rows[0].clone();
        duplicate["id"] = json!(9);
        let mut duplicates = snapshot.clone();
        duplicates.rows.push(duplicate);
        let plan = plan_snapshot(duplicates, &config).unwrap();
        assert!(
            plan.actions
                .iter()
                .all(|a| a.public["status"] == "conflict")
        );
        let mut params = rcgen::CertificateParams::new(vec!["sync.example.test".into()]).unwrap();
        params.not_before = rcgen::date_time_ymd(2000, 1, 1);
        params.not_after = rcgen::date_time_ymd(2001, 1, 1);
        let key = rcgen::KeyPair::generate().unwrap();
        config["ssl"]["certificates"][0]["cert"] = json!(params.self_signed(&key).unwrap().pem());
        config["ssl"]["certificates"][0]["key"] = json!(key.serialize_pem());
        assert_eq!(status(snapshot, &config), "source_invalid");
    }

    #[test]
    fn registry_survives_serialization_without_changing_the_plan() {
        let (mut snapshot, config) = fixture();
        managed(&mut snapshot);
        let registry = serde_json::to_vec(&snapshot.registry).unwrap();
        let before = plan_snapshot(snapshot.clone(), &config).unwrap().version;
        snapshot.registry = serde_json::from_slice(&registry).unwrap();
        let restored = plan_snapshot(snapshot, &config).unwrap();
        assert_eq!(restored.version, before);
        assert_eq!(restored.actions[0].public["status"], "up_to_date");
    }

    #[test]
    fn unfinished_transactions_are_never_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let dummy = FileChange {
            path: String::new(),
            before: None,
            after: None,
            original_metadata: None,
        };
        for i in 0..13 {
            let journal = Journal {
                completed: i != 0,
                files: vec![],
                rows: vec![],
                registry: dummy.clone(),
            };
            save_journal(
                &dir.path()
                    .join(format!("fnos-certificate-sync/transactions/{i:03}.json")),
                &journal,
                dir.path(),
            )
            .unwrap();
        }
        prune_transactions(dir.path()).unwrap();
        assert!(
            dir.path()
                .join("fnos-certificate-sync/transactions/000.json")
                .exists()
        );
        assert!(pending_recovery(dir.path()).unwrap());
        assert_eq!(
            fs::read_dir(dir.path().join("fnos-certificate-sync/transactions"))
                .unwrap()
                .count(),
            11
        );
    }
    #[test]
    fn default_binding_changes_do_not_block_renewal_but_still_block_deletion() {
        let (mut snapshot, config) = fixture();
        managed(&mut snapshot);
        snapshot.rows[0]["is_default"] = json!(1);
        snapshot.rows[0]["updated_time"] = json!(2);
        snapshot.index[0]["used"] = json!(true);
        assert_eq!(status(snapshot.clone(), &config), "up_to_date");
        assert_eq!(
            status(snapshot, &json!({"ssl":{"certificates":[]}})),
            "delete_blocked"
        );
    }
    #[test]
    fn legacy_registry_digests_migrate_only_when_the_old_snapshot_matches() {
        let (mut snapshot, config) = fixture();
        managed(&mut snapshot);
        let old = target_digest_version(&snapshot, &snapshot.rows[0], 1).unwrap();
        let entry = snapshot.registry.entries.get_mut("8").unwrap();
        entry.digest_version = 1;
        entry.target_digest = old;
        assert_eq!(
            plan_snapshot(snapshot.clone(), &config).unwrap().actions[0].kind,
            "update"
        );
        snapshot.rows[0]["des"] = json!("external modification");
        assert_eq!(status(snapshot, &config), "conflict");
    }

    #[test]
    fn splitting_a_managed_san_group_does_not_hide_sources() {
        let (mut snapshot, mut config) = fixture();
        managed(&mut snapshot);
        let mut second = config["ssl"]["certificates"][0].clone();
        second["id"] = json!("local-2");
        config["ssl"]["certificates"]
            .as_array_mut()
            .unwrap()
            .push(second);
        snapshot
            .registry
            .entries
            .get_mut("8")
            .unwrap()
            .source_ids
            .push("local-2".into());
        let changed = generate_simple_self_signed(vec!["split.example.test".into()]).unwrap();
        config["ssl"]["certificates"][0]["cert"] = json!(changed.cert.pem());
        config["ssl"]["certificates"][0]["key"] = json!(changed.signing_key.serialize_pem());
        let plan = plan_snapshot(snapshot.clone(), &config).unwrap();
        assert!(
            plan.actions
                .iter()
                .any(|a| a.kind == "create" && a.source_ids == vec!["local-1"])
        );
        assert!(
            plan.actions
                .iter()
                .any(|a| a.kind == "update" && a.source_ids == vec!["local-2"])
        );
        let changed = generate_simple_self_signed(vec!["second.example.test".into()]).unwrap();
        config["ssl"]["certificates"][1]["cert"] = json!(changed.cert.pem());
        config["ssl"]["certificates"][1]["key"] = json!(changed.signing_key.serialize_pem());
        let plan = plan_snapshot(snapshot, &config).unwrap();
        assert_eq!(
            plan.actions.iter().filter(|a| a.kind == "create").count(),
            1
        );
        assert_eq!(
            plan.actions.iter().filter(|a| a.kind == "update").count(),
            1
        );
    }

    #[test]
    fn renewal_preserves_primary_domain_until_the_san_changes() {
        let (snapshot, config) = fixture();
        let mut row = snapshot.rows[0].clone();
        let mut index = snapshot.index[0].clone();
        row["domain"] = json!("SYNC.example.test");
        let parsed =
            parse_certificate(config["ssl"]["certificates"][0]["cert"].as_str().unwrap()).unwrap();
        update_domains(&mut row, &mut index, &parsed);
        assert_eq!(row["domain"], "SYNC.example.test");
        let changed = generate_simple_self_signed(vec!["new.example.test".into()]).unwrap();
        update_domains(
            &mut row,
            &mut index,
            &parse_certificate(&changed.cert.pem()).unwrap(),
        );
        assert_eq!(row["domain"], "new.example.test");
    }

    #[cfg(unix)]
    #[test]
    fn deletion_recovery_restores_permissions_and_owner() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fnos-certificate-sync");
        fs::create_dir(&root).unwrap();
        let path = root.join("certificate");
        fs::write(&path, b"original").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let metadata = SavedMetadata::read(&path).unwrap();
        let change = FileChange {
            path: path.to_string_lossy().into(),
            before: Some(BASE64_STANDARD.encode(b"original")),
            after: None,
            original_metadata: Some(metadata.clone()),
        };
        apply_file(&change, false, dir.path()).unwrap();
        assert!(!path.exists());
        apply_file(&change, true, dir.path()).unwrap();
        assert_eq!(SavedMetadata::read(&path).unwrap(), metadata);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(apply_file(&change, true, dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn matching_symlink_content_is_not_accepted_as_an_idempotent_write() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fnos-certificate-sync");
        fs::create_dir(&root).unwrap();
        let target = dir.path().join("outside");
        fs::write(&target, b"same").unwrap();
        let path = root.join("link");
        symlink(&target, &path).unwrap();
        let change = FileChange {
            path: path.to_string_lossy().into(),
            before: None,
            after: Some(BASE64_STANDARD.encode(b"same")),
            original_metadata: None,
        };
        assert!(apply_file(&change, false, dir.path()).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"same");
        let second = tempfile::tempdir().unwrap();
        let outside = second.path().join("outside-dir");
        fs::create_dir(&outside).unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&outside, second.path().join("fnos-certificate-sync")).unwrap();
        assert!(
            ensure_parent(
                &second.path().join("fnos-certificate-sync/registry.json"),
                second.path()
            )
            .is_err()
        );
        assert_eq!(fs::metadata(&outside).unwrap().mode() & 0o777, 0o755);
        use transaction::Backend;
        let absent = FileChange {
            path: second
                .path()
                .join("fnos-certificate-sync/absent")
                .to_string_lossy()
                .into(),
            before: None,
            after: None,
            original_metadata: None,
        };
        assert!(
            transaction::Native {
                data_dir: second.path(),
                selected: &[]
            }
            .check(&absent)
            .is_err()
        );
    }

    #[test]
    fn deletion_rechecks_removed_fullchain_paths_and_new_shared_files() {
        let (mut current, _) = fixture();
        let row = current.rows[0].clone();
        current.index[0]["fullchain"] = json!("/fullchain");
        let encoded = Some(BASE64_STANDARD.encode(serde_json::to_vec(&current.index).unwrap()));
        let journal = Journal {
            completed: false,
            files: vec![
                FileChange {
                    path: NETWORK_CERT_INDEX.into(),
                    before: encoded.clone(),
                    after: Some(BASE64_STANDARD.encode(b"[]")),
                    original_metadata: None,
                },
                FileChange {
                    path: "/key".into(),
                    before: Some("original".into()),
                    after: None,
                    original_metadata: None,
                },
            ],
            rows: vec![RowChange {
                table: "cert".into(),
                id: 8,
                before: Some(row.clone()),
                after: None,
            }],
            registry: FileChange {
                path: "registry".into(),
                before: None,
                after: None,
                original_metadata: None,
            },
        };
        current.rows.clear();
        current.index.clear();
        assert!(ensure_removals_unreferenced(&journal, &current, false).is_ok());
        current
            .gateway
            .push(json!({"host":"late.example.test","cert":"/fullchain","key":"/other-key"}));
        assert!(ensure_removals_unreferenced(&journal, &current, false).is_err());
        current.gateway.clear();
        let mut other = row;
        other["id"] = json!(9);
        current.rows.push(other);
        assert!(ensure_removals_unreferenced(&journal, &current, false).is_err());
        let mut creation = journal.clone();
        creation.rows[0].after = creation.rows[0].before.take();
        current.rows.clear();
        current
            .gateway
            .push(json!({"host":"late.example.test","cert":"/fullchain","key":"/other-key"}));
        assert!(ensure_removals_unreferenced(&creation, &current, true).is_err());
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use rcgen::generate_simple_self_signed;

    /// Explicit opt-in: runs only the generated .invalid certificate against a real fnOS host.
    #[test]
    #[ignore = "requires root on fnOS and FN_KNOCK_CERT_SYNC_LIVE=1"]
    fn fnos_live_certificate_roundtrip() {
        assert_eq!(std::env::var("FN_KNOCK_CERT_SYNC_LIVE").as_deref(), Ok("1"));
        let data_dir = tempfile::tempdir().unwrap().keep();
        eprintln!("Live test recovery directory: {}", data_dir.display());
        let before = snapshot(&data_dir).unwrap();
        let domain = format!("fn-knock-sync-{}.invalid", Uuid::new_v4());
        let cert = generate_simple_self_signed(vec![domain.clone()]).unwrap();
        let mut config = json!({"ssl":{"certificates":[{"id":"live-fixture","label":"fn-knock isolated live test","cert":cert.cert.pem(),"key":cert.signing_key.serialize_pem(),"updated_at":"1"}]}});
        let result = (|| -> anyhow::Result<()> {
            let legacy = execute(&data_dir, &config, None, None, Some(&[]))?;
            assert_eq!(legacy.synced, 0);
            anyhow::ensure!(
                snapshot(&data_dir)?.rows == before.rows,
                "Legacy request changed existing certificates"
            );
            let p = plan(&data_dir, &config)?;
            let ids = p
                .actions
                .iter()
                .filter(|a| a.kind == "create")
                .map(|a| a.id.clone())
                .collect::<Vec<_>>();
            assert_eq!(ids, vec!["source:live-fixture"]);
            let summary = execute(&data_dir, &config, Some(&ids), Some(&p.version), None)?;
            assert_eq!(summary.created, 1);
            let p = plan(&data_dir, &config)?;
            let row = p
                .actions
                .iter()
                .find(|a| a.public["domain"] == domain)
                .unwrap();
            assert_eq!(row.public["status"], "up_to_date");
            let target_id = row.row.as_ref().unwrap().id;
            let cert = generate_simple_self_signed(vec![domain.clone()]).unwrap();
            config["ssl"]["certificates"][0]["cert"] = json!(cert.cert.pem());
            config["ssl"]["certificates"][0]["key"] = json!(cert.signing_key.serialize_pem());
            let p = plan(&data_dir, &config)?;
            let summary = execute(
                &data_dir,
                &config,
                Some(&[format!("target:{target_id}")]),
                Some(&p.version),
                None,
            )?;
            assert_eq!(summary.updated, 1);
            // Exercise real crash recovery after deletion, with a non-default certificate mode.
            let current = snapshot(&data_dir)?;
            let fixture_cert = raw_row(&current, target_id)?["certificate"]
                .as_str()
                .ok_or_else(|| anyhow!("Missing fixture certificate path"))?
                .to_string();
            #[cfg(unix)]
            fs::set_permissions(&fixture_cert, fs::Permissions::from_mode(0o640))?;
            config["ssl"]["certificates"] = json!([]);
            let deletion = plan(&data_dir, &config)?;
            let selected = deletion
                .actions
                .iter()
                .filter(|a| a.id == format!("target:{target_id}"))
                .collect::<Vec<_>>();
            let journal = prepare(&deletion, &selected, &data_dir)?;
            let recovery =
                data_dir.join("fnos-certificate-sync/transactions/live-crash-recovery.json");
            save_journal(&recovery, &journal, &data_dir)?;
            transaction::apply(
                &journal,
                &mut transaction::Native {
                    data_dir: &data_dir,
                    selected: &selected,
                },
            )?;
            anyhow::ensure!(
                !Path::new(&fixture_cert).exists(),
                "Fixture deletion did not remove the file"
            );
            // No completion marker: reproduce a process stopping after application of the journal.
            recover(&data_dir)?;
            #[cfg(unix)]
            anyhow::ensure!(
                fs::metadata(&fixture_cert)?.mode() & 0o777 == 0o640,
                "Recovery changed certificate permissions"
            );
            anyhow::ensure!(
                snapshot(&data_dir)?
                    .rows
                    .iter()
                    .any(|r| r["id"].as_i64() == Some(target_id)),
                "Recovery did not restore fixture row"
            );
            let p = plan(&data_dir, &config)?;
            let summary = execute(
                &data_dir,
                &config,
                Some(&[format!("target:{target_id}")]),
                Some(&p.version),
                None,
            )?;
            assert_eq!(summary.deleted, 1);
            let after = snapshot(&data_dir)?;
            anyhow::ensure!(before.rows == after.rows, "Existing fnOS rows changed");
            anyhow::ensure!(before.index == after.index, "Existing fnOS index changed");
            anyhow::ensure!(before.files == after.files, "Existing fnOS files changed");
            anyhow::ensure!(before.used == after.used, "Existing fnOS used changed");
            anyhow::ensure!(before.renew == after.renew, "Existing fnOS renew changed");
            anyhow::ensure!(
                before.gateway == after.gateway,
                "Existing fnOS gateway changed"
            );
            for mapping in &before.gateway {
                let path = mapping["cert"]
                    .as_str()
                    .ok_or_else(|| anyhow!("Missing mapped certificate"))?;
                let pem = before
                    .files
                    .get(path)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| anyhow!("Missing certificate snapshot"))?;
                let expected =
                    parse_certificate(&String::from_utf8(BASE64_STANDARD.decode(pem)?)?)?;
                verify_host(mapping["host"].as_str().unwrap_or("fallback"), &expected)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            // Keep the recoverable journal available instead of losing it with TempDir cleanup.
            let recovery_path = &data_dir;
            panic!(
                "Live certificate verification failed: {error}; recovery directory: {}",
                recovery_path.display()
            );
        }
        fs::remove_dir_all(&data_dir).unwrap();
    }
}

/// Recheck live references at mutation boundaries; include paths removed from the current index.
fn ensure_removals_unreferenced(
    journal: &Journal,
    current: &Snapshot,
    reverse: bool,
) -> anyhow::Result<()> {
    let index_change = journal.files.iter().find(|f| f.path == NETWORK_CERT_INDEX);
    let mut historical = current.index.clone();
    if let Some(change) = index_change {
        for value in [&change.before, &change.after].into_iter().flatten() {
            historical.extend(serde_json::from_slice::<Vec<Value>>(
                &BASE64_STANDARD.decode(value)?,
            )?);
        }
    }
    let deleted_files = journal
        .files
        .iter()
        .filter(|f| {
            if reverse {
                f.before.is_none()
            } else {
                f.after.is_none()
            }
        })
        .map(|f| f.path.as_str())
        .collect::<BTreeSet<_>>();
    for change in journal.rows.iter().filter(|r| r.table == "cert") {
        let (removed, replacement) = if reverse {
            (&change.after, &change.before)
        } else {
            (&change.before, &change.after)
        };
        if replacement.is_some() {
            continue;
        }
        let Some(row) = removed else {
            continue;
        };
        let paths = file_paths(row, &historical);
        if !references(current, row).is_empty()
            || current.gateway.iter().any(|mapping| {
                ["cert", "key"].iter().any(|key| {
                    mapping[key]
                        .as_str()
                        .is_some_and(|path| paths.contains(path))
                })
            })
        {
            bail!("Certificate acquired a service or gateway reference; deletion stopped")
        }
        if current
            .rows
            .iter()
            .filter(|r| r["id"] != row["id"])
            .any(|other| {
                file_paths(other, &current.index)
                    .iter()
                    .any(|path| deleted_files.contains(path.as_str()))
            })
        {
            bail!("Certificate files became shared; deletion stopped")
        }
    }
    Ok(())
}
fn verify_restored(journal: &Journal, data_dir: &Path) -> anyhow::Result<()> {
    let current = snapshot(data_dir)?;
    for change in &journal.rows {
        let rows = if change.table == "cert" {
            &current.rows
        } else {
            &current.renew
        };
        if rows
            .iter()
            .find(|row| row["id"].as_i64() == Some(change.id))
            != change.before.as_ref()
        {
            bail!("Restored certificate database verification failed")
        }
    }
    for file in journal
        .files
        .iter()
        .chain(std::iter::once(&journal.registry))
    {
        check_file(file, true)?;
        if read_optional(Path::new(&file.path))? != file.before {
            bail!("Restored certificate file verification failed")
        }
    }
    Ok(())
}
