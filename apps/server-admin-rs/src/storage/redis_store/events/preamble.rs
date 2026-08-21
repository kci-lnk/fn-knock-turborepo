use super::*;
pub(super) fn system_event_command_tx(
    tx: &Transaction<'_>,
    command: &str,
    args: Vec<String>,
) -> crate::storage::StorageResult<redis::CmdOutput> {
    redis::execute_command_in_transaction(tx, command, args)
}

pub(super) fn command_ok_tx(
    tx: &Transaction<'_>,
    command: &str,
    args: Vec<String>,
) -> crate::storage::StorageResult<()> {
    let _ = system_event_command_tx(tx, command, args)?;
    Ok(())
}
