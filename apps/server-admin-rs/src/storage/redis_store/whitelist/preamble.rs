use super::*;

pub(super) const WHITELIST_MUTATION_MAX_RETRIES: usize = 8;

pub(super) enum TypedWhitelistMutation {
    Upsert(TypedWhitelistDocument),
    Delete {
        kind: &'static str,
        id: String,
    },
    ReplaceKind {
        kind: &'static str,
        documents: Vec<TypedWhitelistDocument>,
    },
}

pub(super) fn typed_whitelist_record(
    record: &WhitelistRecord,
) -> crate::storage::StorageResult<TypedWhitelistDocument> {
    Ok(TypedWhitelistDocument {
        kind: "record",
        id: record.id.clone(),
        document_json: serde_json::to_string(record)?,
        sort_score: record.created_at,
        expires_at: record.expire_at,
        status: record.status.clone(),
    })
}

pub(super) fn typed_whitelist_region(
    record: &WhitelistRegionGroupRecord,
) -> crate::storage::StorageResult<TypedWhitelistDocument> {
    Ok(TypedWhitelistDocument {
        kind: "region",
        id: record.id.clone(),
        document_json: serde_json::to_string(record)?,
        sort_score: record.created_at,
        expires_at: record.expire_at,
        status: record.status.clone(),
    })
}

pub(super) fn whitelist_record_from_typed(
    document: TypedWhitelistDocument,
) -> crate::storage::StorageResult<WhitelistRecord> {
    let record = deserialize_whitelist_record(&document.document_json).ok_or_else(|| {
        crate::storage::storage_error(format!(
            "typed whitelist record {} is malformed",
            document.id
        ))
    })?;
    if record.id != document.id
        || record.created_at != document.sort_score
        || record.expire_at != document.expires_at
        || record.status != document.status
    {
        return Err(crate::storage::storage_error(format!(
            "typed whitelist record {} metadata mismatch",
            document.id
        )));
    }
    Ok(record)
}

pub(super) fn whitelist_region_from_typed(
    document: TypedWhitelistDocument,
) -> crate::storage::StorageResult<WhitelistRegionGroupRecord> {
    let record = deserialize_whitelist_region_group(&document.document_json).ok_or_else(|| {
        crate::storage::storage_error(format!(
            "typed whitelist region {} is malformed",
            document.id
        ))
    })?;
    if record.id != document.id
        || record.created_at != document.sort_score
        || record.expire_at != document.expires_at
        || record.status != document.status
    {
        return Err(crate::storage::storage_error(format!(
            "typed whitelist region {} metadata mismatch",
            document.id
        )));
    }
    Ok(record)
}
