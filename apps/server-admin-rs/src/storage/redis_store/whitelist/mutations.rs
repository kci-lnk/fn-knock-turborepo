use super::*;

pub(super) fn apply_typed_whitelist_mutation(
    tx: &tokio_rusqlite::rusqlite::Transaction<'_>,
    mutation: TypedWhitelistMutation,
) -> crate::storage::StorageResult<()> {
    match mutation {
        TypedWhitelistMutation::Upsert(document) => {
            TypedWhitelistRepository::upsert_tx(tx, &document)
        }
        TypedWhitelistMutation::Delete { kind, id } => {
            TypedWhitelistRepository::delete_tx(tx, kind, &id)
        }
        TypedWhitelistMutation::ReplaceKind { kind, documents } => {
            TypedWhitelistRepository::delete_kind_tx(tx, kind)?;
            for document in &documents {
                TypedWhitelistRepository::upsert_tx(tx, document)?;
            }
            Ok(())
        }
    }
}
