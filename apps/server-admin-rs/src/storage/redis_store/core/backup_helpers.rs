use super::*;

pub(super) fn append_backup_restore_commands(pipe: &mut redis::Pipeline, entry: &Value) -> usize {
    let key = entry.get("key").and_then(Value::as_str).unwrap_or("");
    let value_type = entry.get("type").and_then(Value::as_str).unwrap_or("");
    let ttl_ms = entry
        .get("ttl_ms")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0);
    if key.is_empty() {
        return 0;
    }

    let mut command_count = 0usize;
    match value_type {
        "string" => {
            let command = pipe
                .cmd("SET")
                .arg(key)
                .arg(entry.get("value").and_then(Value::as_str).unwrap_or(""));
            if let Some(ttl_ms) = ttl_ms {
                command.arg("PX").arg(ttl_ms);
            }
            command.ignore();
            command_count += 1;
        }
        "hash" => {
            if let Some(object) = entry.get("value").and_then(Value::as_object)
                && !object.is_empty()
            {
                let pairs = object
                    .iter()
                    .filter_map(|(field, value)| value.as_str().map(|text| (field.as_str(), text)))
                    .collect::<Vec<_>>();
                if pairs.is_empty() {
                    return command_count;
                }
                pipe.cmd("HSET").arg(key);
                for (field, value) in pairs {
                    pipe.arg(field).arg(value);
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "list" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array)
                && !items.is_empty()
            {
                pipe.cmd("RPUSH").arg(key);
                for item in items {
                    pipe.arg(item.as_str().unwrap_or(""));
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "set" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array)
                && !items.is_empty()
            {
                pipe.cmd("SADD").arg(key);
                for item in items {
                    pipe.arg(item.as_str().unwrap_or(""));
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "zset" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array)
                && !items.is_empty()
            {
                pipe.cmd("ZADD").arg(key);
                for item in items {
                    pipe.arg(item.get("score").and_then(Value::as_f64).unwrap_or(0.0))
                        .arg(item.get("member").and_then(Value::as_str).unwrap_or(""));
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "stream" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array) {
                for item in items {
                    let id = item.get("id").and_then(Value::as_str).unwrap_or("*");
                    let Some(fields) = item.get("fields").and_then(Value::as_array) else {
                        continue;
                    };
                    if fields.is_empty() || fields.len() % 2 != 0 {
                        continue;
                    }
                    pipe.cmd("XADD").arg(key).arg(id);
                    for field in fields {
                        pipe.arg(field.as_str().unwrap_or(""));
                    }
                    pipe.ignore();
                    command_count += 1;
                }
            }
        }
        _ => {}
    }

    if let Some(ttl_ms) = ttl_ms.filter(|_| !matches!(value_type, "none" | "string")) {
        pipe.cmd("PEXPIRE").arg(key).arg(ttl_ms).ignore();
        command_count += 1;
    }
    command_count
}
