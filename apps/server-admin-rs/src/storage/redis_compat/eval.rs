use super::*;

pub(super) fn eval_command_tx(
    tx: &rusqlite::Transaction<'_>,
    args: &[String],
) -> RedisResult<CmdOutput> {
    let script = arg(args, 0)?;
    let key_count = usize::try_from(parse_i64(arg(args, 1)?)?)
        .map_err(|_| storage_error("EVAL key count must be non-negative"))?;
    let keys_start = 2_usize;
    let argv_start = keys_start
        .checked_add(key_count)
        .filter(|index| *index <= args.len())
        .ok_or_else(|| storage_error("EVAL key count exceeds supplied arguments"))?;
    let keys = &args[keys_start..argv_start];
    let argv = &args[argv_start..];

    if script.contains("fn-knock:eval:increment-counter-with-ttl:v1") {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("counter EVAL key missing"))?;
        let ttl = parse_i64(
            argv.first()
                .ok_or_else(|| storage_error("counter EVAL TTL missing"))?,
        )?
        .max(1);
        let current = match string_get_tx(tx, key)? {
            Some(value) => value
                .parse::<i64>()
                .map_err(|_| storage_error("counter value is not an integer"))?,
            None => 0,
        };
        let next = current
            .checked_add(1)
            .ok_or_else(|| storage_error("counter overflow"))?;
        if next == 1 {
            set_string_tx(
                tx,
                key,
                &next.to_string(),
                Some(now_ms().saturating_add(ttl.saturating_mul(1000))),
            )?;
        } else {
            set_string_preserve_ttl_tx(tx, key, &next.to_string())?;
        }
        return Ok(CmdOutput::Int(next));
    }

    if script.contains("fn-knock:eval:set-expiring-string-with-zset-limit:v1") {
        let data_key = keys
            .first()
            .ok_or_else(|| storage_error("limited string EVAL data key missing"))?;
        let index_key = keys
            .get(1)
            .ok_or_else(|| storage_error("limited string EVAL index key missing"))?;
        let value = argv
            .first()
            .ok_or_else(|| storage_error("limited string EVAL value missing"))?;
        let ttl = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("limited string EVAL TTL missing"))?,
        )?
        .max(1);
        let now_score = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("limited string EVAL current score missing"))?,
        )?;
        let expires_at_score = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("limited string EVAL expiry score missing"))?,
        )?;
        let limit = parse_i64(
            argv.get(4)
                .ok_or_else(|| storage_error("limited string EVAL limit missing"))?,
        )?
        .max(1);

        purge_expired_tx(tx, index_key)?;
        delete_zset_score_range_tx(
            tx,
            index_key,
            ScoreBound::inclusive(f64::NEG_INFINITY),
            ScoreBound::inclusive(now_score as f64),
        )?;
        let tracked = count_rows_tx(
            tx,
            "SELECT COUNT(*) FROM kv_zset WHERE key = ?1 AND member = ?2",
            &[index_key, data_key],
        )? > 0;
        let existing = string_get_tx(tx, data_key)?.is_some();
        let active = count_rows_tx(
            tx,
            "SELECT COUNT(*) FROM kv_zset WHERE key = ?1",
            &[index_key],
        )?;
        if !tracked && !existing && active >= limit {
            return Ok(CmdOutput::Int(0));
        }

        set_string_tx(
            tx,
            data_key,
            value,
            Some(now_ms().saturating_add(ttl.saturating_mul(1000))),
        )?;
        ensure_key_tx(tx, index_key, "zset", None)?;
        tx.execute(
            "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
            params![index_key, data_key, expires_at_score],
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:claim-ldap-binding:v2") {
        if keys.len() < 4 || argv.len() < 5 {
            return Err(storage_error("LDAP binding EVAL arguments missing"));
        }
        let Some(invite_raw) = string_get_tx(tx, &keys[0])? else {
            return Ok(CmdOutput::Int(0));
        };
        let Ok(invite) = serde_json::from_str::<serde_json::Value>(&invite_raw) else {
            return Ok(CmdOutput::Int(0));
        };
        if invite.get("used_at").is_some()
            || invite
                .get("provider_id")
                .and_then(serde_json::Value::as_str)
                != Some(argv[3].as_str())
            || invite.get("totp_id").and_then(serde_json::Value::as_str) != Some(argv[4].as_str())
            || string_get_tx(tx, &keys[1])?.is_some()
            || string_get_tx(tx, &keys[2])?.is_some()
        {
            return Ok(CmdOutput::Int(0));
        }
        let score = argv[2]
            .parse::<f64>()
            .map_err(|_| storage_error("LDAP binding EVAL score is invalid"))?;
        set_string_tx(tx, &keys[1], &argv[0], None)?;
        set_string_tx(tx, &keys[2], &argv[1], None)?;
        ensure_key_tx(tx, &keys[3], "zset", None)?;
        tx.execute(
            "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
            params![&keys[3], &argv[0], score],
        )?;
        delete_key_tx(tx, &keys[0])?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:claim-oidc-binding:v1") {
        if keys.len() < 4 || argv.len() < 5 {
            return Err(storage_error("OIDC binding EVAL arguments missing"));
        }
        let Some(invite_raw) = string_get_tx(tx, &keys[0])? else {
            return Ok(CmdOutput::Int(0));
        };
        let Ok(invite) = serde_json::from_str::<serde_json::Value>(&invite_raw) else {
            return Ok(CmdOutput::Int(0));
        };
        let current_binding = string_get_tx(tx, &keys[1])?;
        if invite
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            != Some(argv[3].as_str())
            || invite.get("totp_id").and_then(serde_json::Value::as_str) != Some(argv[4].as_str())
            || current_binding
                .as_deref()
                .is_some_and(|binding_id| binding_id != argv[0])
        {
            return Ok(CmdOutput::Int(0));
        }
        let score = argv[2]
            .parse::<f64>()
            .map_err(|_| storage_error("OIDC binding EVAL score is invalid"))?;
        set_string_tx(tx, &keys[1], &argv[0], None)?;
        set_string_tx(tx, &keys[2], &argv[1], None)?;
        ensure_key_tx(tx, &keys[3], "zset", None)?;
        tx.execute(
            "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
            params![&keys[3], &argv[0], score],
        )?;
        delete_key_tx(tx, &keys[0])?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:update-owned-binding:v1") {
        if keys.len() < 3 || argv.len() < 3 {
            return Err(storage_error("binding update EVAL arguments missing"));
        }
        if string_get_tx(tx, &keys[0])?.as_deref() != Some(argv[0].as_str())
            || string_get_tx(tx, &keys[1])?.is_none()
        {
            return Ok(CmdOutput::Int(0));
        }
        let score = argv[2]
            .parse::<f64>()
            .map_err(|_| storage_error("LDAP binding update EVAL score is invalid"))?;
        set_string_tx(tx, &keys[1], &argv[1], None)?;
        ensure_key_tx(tx, &keys[2], "zset", None)?;
        tx.execute(
            "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
            params![&keys[2], &argv[0], score],
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:delete-owned-binding:v1") {
        if keys.len() < 3 || argv.is_empty() {
            return Err(storage_error("binding delete EVAL arguments missing"));
        }
        if string_get_tx(tx, &keys[1])?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        delete_key_tx(tx, &keys[1])?;
        if string_get_tx(tx, &keys[0])?.as_deref() == Some(argv[0].as_str()) {
            delete_key_tx(tx, &keys[0])?;
        }
        tx.execute(
            "DELETE FROM kv_zset WHERE key = ?1 AND member = ?2",
            params![&keys[2], &argv[0]],
        )?;
        delete_collection_key_if_empty_tx(tx, &keys[2], "zset")?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:cas-config-host-generation-raw:v3") {
        let config_key = keys
            .first()
            .ok_or_else(|| storage_error("config CAS config key missing"))?;
        let generation_key = keys
            .get(1)
            .ok_or_else(|| storage_error("config CAS generation key missing"))?;
        let config_expected_exists = argv
            .first()
            .ok_or_else(|| storage_error("config CAS config exists flag missing"))?;
        let config_expected_raw = argv
            .get(1)
            .ok_or_else(|| storage_error("config CAS expected config missing"))?;
        let generation_expected_exists = argv
            .get(2)
            .ok_or_else(|| storage_error("config CAS generation exists flag missing"))?;
        let generation_expected_raw = argv
            .get(3)
            .ok_or_else(|| storage_error("config CAS expected generation missing"))?;
        let replacement_config_raw = argv
            .get(4)
            .ok_or_else(|| storage_error("config CAS replacement config missing"))?;
        let replacement_generation_raw = argv
            .get(5)
            .ok_or_else(|| storage_error("config CAS replacement generation missing"))?;

        let read_raw = |key: &str| -> RedisResult<Option<String>> {
            purge_expired_tx(tx, key)?;
            match key_kind_tx(tx, key)? {
                None => Ok(None),
                Some(kind) if kind == "string" => string_get_tx(tx, key),
                Some(_) => Err(storage_error("config CAS key must contain a string")),
            }
        };
        let raw_matches = |current: Option<&str>, exists: &str, expected: &str| match exists {
            "0" => Ok(current.is_none()),
            "1" => Ok(current == Some(expected)),
            _ => Err(storage_error("config CAS exists flag is invalid")),
        };
        let current_config_raw = read_raw(config_key)?;
        let current_generation_raw = read_raw(generation_key)?;
        if !raw_matches(
            current_config_raw.as_deref(),
            config_expected_exists,
            config_expected_raw,
        )? || !raw_matches(
            current_generation_raw.as_deref(),
            generation_expected_exists,
            generation_expected_raw,
        )? {
            return Ok(CmdOutput::Int(0));
        }
        set_string_tx(tx, config_key, replacement_config_raw, None)?;
        set_string_tx(tx, generation_key, replacement_generation_raw, None)?;
        let replacement_generation = replacement_generation_raw
            .parse::<u64>()
            .map_err(|_| storage_error("config CAS replacement generation is invalid"))?;
        let typed_revision = crate::storage::typed_config::upsert_config_document_tx(
            tx,
            replacement_config_raw,
            replacement_generation,
        )?;
        let typed_revision = i64::try_from(typed_revision)
            .map_err(|_| storage_error("typed config revision exceeds SQLite range"))?;
        return Ok(CmdOutput::Int(typed_revision));
    }

    if script.contains("fn-knock:eval:update-json-cas:v1")
        || script.contains("fn-knock:eval:update-session-json-cas:v2")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("JSON update EVAL key missing"))?;
        let expected_raw = argv
            .first()
            .ok_or_else(|| storage_error("JSON update EVAL expected value missing"))?;
        let next_raw = argv
            .get(1)
            .ok_or_else(|| storage_error("JSON update EVAL next value missing"))?;
        let Some(current_raw) = string_get_tx(tx, key)? else {
            return Ok(CmdOutput::Int(-1));
        };
        if current_raw != *expected_raw {
            return Ok(CmdOutput::Int(0));
        }
        let changed = tx.execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            params![key, next_raw],
        )?;
        if changed == 0 {
            return Ok(CmdOutput::Int(-1));
        }
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:initialize-login-mobility-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("login mobility EVAL session key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let binding_key = keys
            .get(1)
            .ok_or_else(|| storage_error("login mobility EVAL binding key missing"))?;
        let timeline_key = keys
            .get(2)
            .ok_or_else(|| storage_error("login mobility EVAL timeline key missing"))?;
        let summary_key = keys
            .get(3)
            .ok_or_else(|| storage_error("login mobility EVAL summary key missing"))?;
        let index_key = keys
            .get(4)
            .ok_or_else(|| storage_error("login mobility EVAL index key missing"))?;
        let whitelist_owner_key = keys
            .get(5)
            .ok_or_else(|| storage_error("login mobility EVAL whitelist owner key missing"))?;
        let ttl = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("login mobility EVAL TTL missing"))?,
        )?
        .max(1);
        for (key, value_index) in [
            (binding_key, 0_usize),
            (timeline_key, 1_usize),
            (summary_key, 2_usize),
        ] {
            let value = argv
                .get(value_index)
                .ok_or_else(|| storage_error("login mobility EVAL value missing"))?;
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "SETEX".to_string(),
                    args: vec![key.clone(), ttl.to_string(), value.clone()],
                    ignore: false,
                },
            )?;
        }
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SADD".to_string(),
                args: vec![index_key.clone(), binding_key.clone()],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "EXPIRE".to_string(),
                args: vec![index_key.clone(), ttl.to_string()],
                ignore: false,
            },
        )?;
        let session_id = argv
            .get(4)
            .ok_or_else(|| storage_error("login mobility EVAL session ID missing"))?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SETEX".to_string(),
                args: vec![
                    whitelist_owner_key.clone(),
                    ttl.to_string(),
                    session_id.clone(),
                ],
                ignore: false,
            },
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:add-pending-whitelist-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("pending whitelist EVAL session key missing"))?;
        let pending_key = keys
            .get(1)
            .ok_or_else(|| storage_error("pending whitelist EVAL pending key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let record_id = argv
            .first()
            .ok_or_else(|| storage_error("pending whitelist EVAL record ID missing"))?;
        let owner_record_key = argv
            .get(1)
            .ok_or_else(|| storage_error("pending whitelist EVAL owner key missing"))?;
        let ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("pending whitelist EVAL TTL missing"))?,
        )?
        .max(1);
        execute_command_tx(
            tx,
            CommandSpec {
                name: "HSET".to_string(),
                args: vec![
                    pending_key.clone(),
                    record_id.clone(),
                    owner_record_key.clone(),
                ],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "EXPIRE".to_string(),
                args: vec![pending_key.clone(), ttl.to_string()],
                ignore: false,
            },
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:save-timeline-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("timeline EVAL session key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let timeline_key = keys
            .get(1)
            .ok_or_else(|| storage_error("timeline EVAL timeline key missing"))?;
        let summary_key = keys
            .get(2)
            .ok_or_else(|| storage_error("timeline EVAL summary key missing"))?;
        let events = argv
            .first()
            .ok_or_else(|| storage_error("timeline EVAL events missing"))?;
        let summary = argv
            .get(1)
            .ok_or_else(|| storage_error("timeline EVAL summary missing"))?;
        let ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("timeline EVAL TTL missing"))?,
        )?;
        let expires_at = (ttl > 0).then(|| now_ms().saturating_add(ttl.saturating_mul(1000)));
        set_string_tx(tx, timeline_key, events, expires_at)?;
        set_string_tx(tx, summary_key, summary, expires_at)?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:collect-mobility-session-whitelist:v1") {
        if keys.len() < 4 {
            return Err(storage_error("collect mobility EVAL keys missing"));
        }
        let session_id = argv
            .first()
            .ok_or_else(|| storage_error("collect mobility EVAL session ID missing"))?;
        let snapshot = auth_mobility_session_snapshot_tx(
            tx, session_id, &keys[0], &keys[1], &keys[2], &keys[3],
        )?;
        return Ok(CmdOutput::Strings(
            snapshot.whitelist_ids.into_iter().collect(),
        ));
    }

    let destroys_session_authority =
        script.contains("fn-knock:eval:destroy-mobility-session-and-authority:v2");
    if destroys_session_authority || script.contains("fn-knock:eval:destroy-mobility-session:v1") {
        let required_keys = if destroys_session_authority { 9 } else { 7 };
        if keys.len() < required_keys {
            return Err(storage_error("destroy mobility EVAL keys missing"));
        }
        let session_id = argv
            .first()
            .ok_or_else(|| storage_error("destroy mobility EVAL session ID missing"))?;
        let owner_prefix = argv
            .get(1)
            .ok_or_else(|| storage_error("destroy mobility EVAL owner prefix missing"))?;
        let snapshot = auth_mobility_session_snapshot_tx(
            tx, session_id, &keys[0], &keys[1], &keys[5], &keys[6],
        )?;
        for binding_key in snapshot.owned_binding_keys {
            delete_key_tx(tx, &binding_key)?;
        }
        for owner_record_key in snapshot.owner_record_keys {
            delete_key_tx(tx, &owner_record_key)?;
        }
        for record_id in &snapshot.whitelist_ids {
            let owner_key = format!("{owner_prefix}{record_id}:session");
            if string_get_tx(tx, &owner_key)?.as_deref() == Some(session_id.as_str()) {
                delete_key_tx(tx, &owner_key)?;
            }
        }
        for key in [&keys[0], &keys[1], &keys[2], &keys[3], &keys[4], &keys[6]] {
            delete_key_tx(tx, key)?;
        }
        if destroys_session_authority {
            delete_key_tx(tx, &keys[7])?;
            delete_key_tx(tx, &keys[8])?;
        }
        return Ok(CmdOutput::Strings(
            snapshot.whitelist_ids.into_iter().collect(),
        ));
    }

    if script.contains("fn-knock:eval:save-active-ip-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("active IP EVAL session key missing"))?;
        let zset_key = keys
            .get(1)
            .ok_or_else(|| storage_error("active IP EVAL zset key missing"))?;
        let detail_key = keys
            .get(2)
            .ok_or_else(|| storage_error("active IP EVAL detail key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let ip = argv
            .first()
            .ok_or_else(|| storage_error("active IP EVAL IP missing"))?;
        let score = argv
            .get(1)
            .ok_or_else(|| storage_error("active IP EVAL score missing"))?;
        let detail = argv
            .get(2)
            .ok_or_else(|| storage_error("active IP EVAL detail missing"))?;
        let ttl = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("active IP EVAL TTL missing"))?,
        )?
        .max(1);
        execute_command_tx(
            tx,
            CommandSpec {
                name: "ZADD".to_string(),
                args: vec![zset_key.clone(), score.clone(), ip.clone()],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "HSET".to_string(),
                args: vec![detail_key.clone(), ip.clone(), detail.clone()],
                ignore: false,
            },
        )?;
        for key in [zset_key, detail_key] {
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "EXPIRE".to_string(),
                    args: vec![key.clone(), ttl.to_string()],
                    ignore: false,
                },
            )?;
        }
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:save-owned-binding-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("owned binding EVAL session key missing"))?;
        let binding_key = keys
            .get(1)
            .ok_or_else(|| storage_error("owned binding EVAL binding key missing"))?;
        let index_key = keys
            .get(2)
            .ok_or_else(|| storage_error("owned binding EVAL index key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let binding = argv
            .first()
            .ok_or_else(|| storage_error("owned binding EVAL value missing"))?;
        let binding_ttl = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("owned binding EVAL TTL missing"))?,
        )?
        .max(1);
        let index_ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("owned binding EVAL index TTL missing"))?,
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SETEX".to_string(),
                args: vec![
                    binding_key.clone(),
                    binding_ttl.to_string(),
                    binding.clone(),
                ],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SADD".to_string(),
                args: vec![index_key.clone(), binding_key.clone()],
                ignore: false,
            },
        )?;
        if index_ttl > 0 {
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "EXPIRE".to_string(),
                    args: vec![index_key.clone(), index_ttl.to_string()],
                    ignore: false,
                },
            )?;
        }
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:save-binding-keep-ttl-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("binding keep-TTL EVAL session key missing"))?;
        let binding_key = keys
            .get(1)
            .ok_or_else(|| storage_error("binding keep-TTL EVAL binding key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() || string_get_tx(tx, binding_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let binding = argv
            .first()
            .ok_or_else(|| storage_error("binding keep-TTL EVAL value missing"))?;
        let changed = tx.execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            params![binding_key, binding],
        )?;
        return Ok(CmdOutput::Int((changed > 0) as i64));
    }

    if script.contains("fn-knock:eval:update-binding-keep-ttl-if-exists:v1") {
        let binding_key = keys
            .first()
            .ok_or_else(|| storage_error("binding update EVAL key missing"))?;
        let index_key = keys
            .get(1)
            .ok_or_else(|| storage_error("binding update EVAL index key missing"))?;
        let Some(current_raw) = string_get_tx(tx, binding_key)? else {
            return Ok(CmdOutput::Int(0));
        };
        let expected_owner = argv
            .get(1)
            .ok_or_else(|| storage_error("binding update EVAL expected owner missing"))?;
        let current_owner = serde_json::from_str::<serde_json::Value>(&current_raw)
            .ok()
            .and_then(|value| {
                value
                    .get("ownerSessionId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            });
        if current_owner.as_deref() != Some(expected_owner.as_str()) {
            return Ok(CmdOutput::Int(0));
        }
        let binding = argv
            .first()
            .ok_or_else(|| storage_error("binding update EVAL value missing"))?;
        let changed = tx.execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            params![binding_key, binding],
        )?;
        if changed > 0 {
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "SREM".to_string(),
                    args: vec![index_key.clone(), binding_key.clone()],
                    ignore: false,
                },
            )?;
        }
        return Ok(CmdOutput::Int((changed > 0) as i64));
    }

    if script.contains("fn-knock:eval:set-whitelist-owner-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("whitelist owner EVAL session key missing"))?;
        let owner_key = keys
            .get(1)
            .ok_or_else(|| storage_error("whitelist owner EVAL owner key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let session_id = argv
            .first()
            .ok_or_else(|| storage_error("whitelist owner EVAL session ID missing"))?;
        let ttl = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("whitelist owner EVAL TTL missing"))?,
        )?
        .max(1);
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SETEX".to_string(),
                args: vec![owner_key.clone(), ttl.to_string(), session_id.clone()],
                ignore: false,
            },
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:json-lock-refresh:v1")
        || script.contains("fn-knock:eval:json-lock-release:v1")
        || script.contains("pcall(cjson.decode, raw)")
            && script.contains("decoded[\"lockId\"] ~= ARGV[1]")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("JSON lock EVAL key missing"))?;
        let expected_lock_id = argv
            .first()
            .ok_or_else(|| storage_error("JSON lock EVAL lock id missing"))?;
        let owned = string_get_tx(tx, key)?
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|value| {
                value
                    .get("lockId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .is_some_and(|lock_id| lock_id == *expected_lock_id);
        if !owned {
            return Ok(CmdOutput::Int(0));
        }

        if script.contains("fn-knock:eval:json-lock-refresh:v1")
            || script.contains("redis.call(\"SET\", KEYS[1], ARGV[2]")
        {
            let value = argv
                .get(1)
                .ok_or_else(|| storage_error("JSON lock EVAL value missing"))?;
            let ttl_seconds = parse_i64(
                argv.get(2)
                    .ok_or_else(|| storage_error("JSON lock EVAL TTL missing"))?,
            )?;
            if ttl_seconds <= 0 {
                return Err(storage_error("JSON lock EVAL TTL must be positive"));
            }
            set_string_tx(
                tx,
                key,
                value,
                Some(now_ms().saturating_add(ttl_seconds.saturating_mul(1000))),
            )?;
            return Ok(CmdOutput::Int(1));
        }

        if script.contains("fn-knock:eval:json-lock-release:v1")
            || script.contains("redis.call(\"DEL\", KEYS[1])")
        {
            delete_key_tx(tx, key)?;
            return Ok(CmdOutput::Int(1));
        }

        return Err(storage_error("unsupported JSON lock EVAL operation"));
    }

    if script.contains("fn-knock:eval:delete-if-value:v1")
        || script.contains("redis.call('GET', KEYS[1]) == ARGV[1]")
        || script.contains("redis.call(\"GET\", KEYS[1])") && script.contains("value == ARGV[1]")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let expected = argv
            .first()
            .ok_or_else(|| storage_error("EVAL argv missing"))?;
        let current = string_get_tx(tx, key)?;
        if current.as_deref() == Some(expected.as_str()) {
            delete_key_tx(tx, key)?;
            return Ok(CmdOutput::Int(1));
        }
        return Ok(CmdOutput::Int(0));
    }

    if script.contains("fn-knock:eval:consume-value:v1")
        || script.contains("local value = redis.call(\"GET\", KEYS[1])")
            && script.contains("redis.call(\"DEL\", KEYS[1])")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let value = string_get_tx(tx, key)?;
        if value.is_some() {
            delete_key_tx(tx, key)?;
        }
        return Ok(CmdOutput::OptionalString(value));
    }

    if script.contains("fn-knock:eval:docker-admin-login-backoff:v1") {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("docker admin login backoff key missing"))?;
        let ip = argv
            .first()
            .ok_or_else(|| storage_error("docker admin login backoff ip missing"))?;
        let now = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("docker admin login backoff now missing"))?,
        )?;
        let now_iso = argv
            .get(2)
            .ok_or_else(|| storage_error("docker admin login backoff timestamp missing"))?;
        let ttl = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("docker admin login backoff TTL missing"))?,
        )?;
        let base_delay = parse_i64(
            argv.get(4)
                .ok_or_else(|| storage_error("docker admin login backoff base missing"))?,
        )?;
        let max_delay = parse_i64(
            argv.get(5)
                .ok_or_else(|| storage_error("docker admin login backoff max missing"))?,
        )?;
        let attempts = string_get_tx(tx, key)?
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|value| value.get("attempts").and_then(serde_json::Value::as_i64))
            .unwrap_or(0)
            .saturating_add(1);
        let exponent = attempts.saturating_sub(1).clamp(0, 30) as u32;
        let backoff_ms = base_delay
            .saturating_mul(2_i64.saturating_pow(exponent))
            .clamp(0, max_delay.max(0));
        let blocked_until = now.saturating_add(backoff_ms);
        let state = serde_json::json!({
            "ip": ip,
            "attempts": attempts,
            "last_attempt_at": now_iso,
            "blocked_until": blocked_until,
        })
        .to_string();
        set_string_tx(
            tx,
            key,
            &state,
            Some(now_ms().saturating_add(ttl.max(1).saturating_mul(1000))),
        )?;
        return Ok(CmdOutput::Ints(vec![
            attempts,
            (backoff_ms + 999) / 1000,
            blocked_until,
        ]));
    }

    if script.contains("fn-knock:eval:login-backoff:v1")
        || script.contains("local key = KEYS[1]") && script.contains("blockedUntil")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let ip = argv
            .first()
            .ok_or_else(|| storage_error("login backoff ip missing"))?;
        let now = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("login backoff now missing"))?,
        )?;
        let ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("login backoff ttl missing"))?,
        )?;
        let base_delay = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("login backoff base missing"))?,
        )?;
        let max_delay = parse_i64(
            argv.get(4)
                .ok_or_else(|| storage_error("login backoff max missing"))?,
        )?;
        let jitter_factor = parse_f64(
            argv.get(5)
                .ok_or_else(|| storage_error("login backoff jitter missing"))?,
        )?;
        let attempts = string_get_tx(tx, key)?
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|value| value.get("attempts").and_then(serde_json::Value::as_i64))
            .unwrap_or(0)
            + 1;
        let exp_delay =
            (2_i64.saturating_pow((attempts - 1).clamp(0, 30) as u32) * base_delay).max(0);
        let seed = format!("{ip}:{attempts}:{now}");
        let mut hash = 0_i64;
        for byte in seed.bytes() {
            hash = (hash * 33 + byte as i64) % 1_000_003;
        }
        let ratio = (hash % 10_000) as f64 / 10_000.0;
        let jitter = ((ratio * 2.0) - 1.0) * (exp_delay as f64 * jitter_factor);
        let backoff_ms = ((exp_delay as f64 + jitter).floor() as i64).clamp(0, max_delay);
        let blocked_until = now + backoff_ms;
        let state = serde_json::json!({
            "ip": ip,
            "attempts": attempts,
            "lastAttempt": now,
            "blockedUntil": blocked_until
        })
        .to_string();
        set_string_tx(tx, key, &state, Some(now_ms() + ttl.max(1) * 1000))?;
        return Ok(CmdOutput::Ints(vec![
            attempts,
            (backoff_ms + 999) / 1000,
            blocked_until,
        ]));
    }

    if script.contains("fn-knock:eval:zset-claim:v1")
        || script.contains("ZRANGEBYSCORE")
            && script.contains("ZREM")
            && script.contains("unpack(ids)")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let max = parse_i64(
            argv.first()
                .ok_or_else(|| storage_error("ready max missing"))?,
        )?;
        let limit = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("ready limit missing"))?,
        )?;
        let ids = zrangebyscore_tx(
            tx,
            key,
            ScoreBound::inclusive(f64::NEG_INFINITY),
            ScoreBound::inclusive(max as f64),
            Some(limit as usize),
            false,
        )?;
        for id in &ids {
            tx.execute(
                "DELETE FROM kv_zset WHERE key = ?1 AND member = ?2",
                params![key, id],
            )?;
        }
        delete_collection_key_if_empty_tx(tx, key, "zset")?;
        return Ok(CmdOutput::Strings(ids));
    }

    Err(storage_error("unsupported Redis-compatible EVAL script"))
}
