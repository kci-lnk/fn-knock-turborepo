use super::*;

pub(super) fn execute_pipeline_commands_tx(
    tx: &rusqlite::Transaction<'_>,
    commands: Vec<CommandSpec>,
) -> RedisResult<Vec<CmdOutput>> {
    let sync_mobility = commands
        .iter()
        .fold(TypedMobilitySyncScope::None, |scope, command| {
            scope.merge(command_typed_mobility_scope(command))
        });
    let mut outputs = Vec::new();
    for command in commands {
        let ignore = command.ignore;
        let output = execute_command_tx(tx, command)?;
        if !ignore {
            outputs.push(output);
        }
    }
    sync_typed_mobility_tx(tx, sync_mobility)?;
    Ok(outputs)
}

/// Compares one compatibility hash field inside a caller-owned transaction.
///
/// The transaction must use `IMMEDIATE` behavior when the result guards a
/// subsequent write. That prevents another connection from changing the
/// compatibility keyspace between this comparison and the caller's commit.
pub(crate) fn hash_field_matches_in_transaction<F>(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    field: &str,
    matches: F,
) -> RedisResult<bool>
where
    F: FnOnce(Option<&str>) -> bool,
{
    purge_expired_tx(tx, key)?;
    let current = tx
        .query_row(
            "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
            params![key, field],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(matches(current.as_deref()))
}

/// Reads a complete compatibility hash inside a caller-owned transaction.
/// Expired keys are removed before the snapshot is returned, matching the
/// public compatibility API.
pub(crate) fn hash_entries_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
) -> RedisResult<Vec<(String, String)>> {
    purge_expired_tx(tx, key)?;
    if key_kind_tx(tx, key)? != Some("hash".to_string()) {
        return Ok(Vec::new());
    }
    let mut statement =
        tx.prepare("SELECT field, value FROM kv_hash WHERE key = ?1 ORDER BY field")?;
    let rows = statement.query_map([key], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Executes one Redis-compatible command inside a caller-owned SQLite
/// transaction. Domain repositories use this to dual-write typed tables and
/// the 2.x compatibility keyspace atomically.
pub(crate) fn execute_command_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    name: &str,
    args: Vec<String>,
) -> RedisResult<CmdOutput> {
    let command = CommandSpec {
        name: name.to_ascii_uppercase(),
        args,
        ignore: false,
    };
    let sync_mobility = command_typed_mobility_scope(&command);
    let output = execute_command_tx(tx, command)?;
    sync_typed_mobility_tx(tx, sync_mobility)?;
    Ok(output)
}
