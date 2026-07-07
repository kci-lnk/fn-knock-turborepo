use super::*;

pub(super) async fn store_list_sessions(
    redis: &RedisStore,
) -> anyhow::Result<Vec<TerminalSessionRecord>> {
    let ids = redis.zrevrange_strings(SESSION_INDEX_KEY).await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let keys = ids
        .iter()
        .map(|id| session_data_key(id))
        .collect::<Vec<_>>();
    let raws = redis.mget_string_values(&keys).await?;
    let mut sessions = Vec::new();
    let mut stale_ids = Vec::new();
    for (index, raw) in raws.into_iter().enumerate() {
        let Some(id) = ids.get(index) else {
            continue;
        };
        let Some(raw) = raw else {
            stale_ids.push(id.clone());
            continue;
        };
        match serde_json::from_str::<TerminalSessionRecord>(&raw) {
            Ok(session) => sessions.push(normalize_session(session)),
            Err(error) => {
                tracing::warn!(session_id = %id, %error, "failed to parse terminal session record");
                stale_ids.push(id.clone());
            }
        }
    }
    for id in stale_ids {
        redis
            .delete_string_and_zrem(&session_data_key(&id), SESSION_INDEX_KEY, &id)
            .await?;
    }
    Ok(sessions)
}

pub(super) async fn store_get_session(
    redis: &RedisStore,
    id: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(raw) = redis.get_string_value(&session_data_key(id)).await? else {
        return Ok(None);
    };
    match serde_json::from_str::<TerminalSessionRecord>(&raw) {
        Ok(session) => Ok(Some(normalize_session(session))),
        Err(error) => {
            tracing::warn!(session_id = %id, %error, "failed to parse terminal session record");
            redis
                .delete_string_and_zrem(&session_data_key(id), SESSION_INDEX_KEY, id)
                .await?;
            Ok(None)
        }
    }
}

pub(super) async fn store_save_session(
    redis: &RedisStore,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let normalized = normalize_session(session);
    let value = serde_json::to_string(&normalized)?;
    let score = parse_iso_ms(&normalized.updated_at).unwrap_or_else(now_ms);
    redis
        .set_string_and_zadd(
            &session_data_key(&normalized.id),
            &value,
            SESSION_INDEX_KEY,
            &normalized.id,
            score,
        )
        .await?;
    Ok(normalized)
}

pub(super) async fn store_delete_session(redis: &RedisStore, id: &str) -> anyhow::Result<()> {
    let attachment_ids = redis.smembers_strings(&session_attachments_key(id)).await?;
    let mut keys = vec![session_data_key(id), session_attachments_key(id)];
    keys.extend(
        attachment_ids
            .iter()
            .map(|attachment_id| attachment_data_key(attachment_id)),
    );
    redis.delete_keys(&keys).await?;
    redis.zrem_string_member(SESSION_INDEX_KEY, id).await?;
    Ok(())
}

pub(super) async fn store_list_attachment_ids_for_session(
    redis: &RedisStore,
    session_id: &str,
) -> anyhow::Result<Vec<String>> {
    let ids = redis
        .smembers_strings(&session_attachments_key(session_id))
        .await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let keys = ids
        .iter()
        .map(|id| attachment_data_key(id))
        .collect::<Vec<_>>();
    let raws = redis.mget_string_values(&keys).await?;
    let mut live_ids = Vec::new();
    let mut stale_ids = Vec::new();
    for (index, raw) in raws.into_iter().enumerate() {
        let Some(id) = ids.get(index) else {
            continue;
        };
        let Some(raw) = raw else {
            stale_ids.push(id.clone());
            continue;
        };
        match serde_json::from_str::<TerminalAttachmentRecord>(&raw) {
            Ok(attachment) => {
                let normalized = normalize_attachment(attachment);
                if normalized.id.is_empty() {
                    stale_ids.push(id.clone());
                } else {
                    live_ids.push(id.clone());
                }
            }
            Err(error) => {
                tracing::warn!(attachment_id = %id, %error, "failed to parse terminal attachment record");
                stale_ids.push(id.clone());
            }
        }
    }
    for id in stale_ids {
        redis
            .delete_string_and_srem(
                &attachment_data_key(&id),
                &session_attachments_key(session_id),
                &id,
            )
            .await?;
    }
    Ok(live_ids)
}

pub(super) async fn store_get_attachment(
    redis: &RedisStore,
    id: &str,
) -> anyhow::Result<Option<TerminalAttachmentRecord>> {
    let Some(raw) = redis.get_string_value(&attachment_data_key(id)).await? else {
        return Ok(None);
    };
    match serde_json::from_str::<TerminalAttachmentRecord>(&raw) {
        Ok(attachment) => Ok(Some(normalize_attachment(attachment))),
        Err(error) => {
            tracing::warn!(attachment_id = %id, %error, "failed to parse terminal attachment record");
            redis.delete_key(&attachment_data_key(id)).await?;
            Ok(None)
        }
    }
}

pub(super) async fn store_save_attachment(
    redis: &RedisStore,
    attachment: TerminalAttachmentRecord,
    ttl_seconds: i64,
) -> anyhow::Result<TerminalAttachmentRecord> {
    let normalized = normalize_attachment(attachment);
    let value = serde_json::to_string(&normalized)?;
    let ttl = ttl_seconds.max(30) as usize;
    redis
        .save_expiring_string_and_sadd(
            &attachment_data_key(&normalized.id),
            &value,
            ttl,
            &session_attachments_key(&normalized.session_id),
            &normalized.id,
        )
        .await?;
    Ok(normalized)
}

pub(super) async fn store_refresh_attachment(
    redis: &RedisStore,
    id: &str,
    ttl_seconds: i64,
) -> anyhow::Result<Option<TerminalAttachmentRecord>> {
    let Some(attachment) = store_get_attachment(redis, id).await? else {
        return Ok(None);
    };
    let next = normalize_attachment(TerminalAttachmentRecord {
        updated_at: now_iso(),
        expires_at: iso_after_seconds(ttl_seconds.max(30)),
        ..attachment
    });
    store_save_attachment(redis, next, ttl_seconds)
        .await
        .map(Some)
}

pub(super) async fn store_delete_attachment(redis: &RedisStore, id: &str) -> anyhow::Result<()> {
    let Some(attachment) = store_get_attachment(redis, id).await? else {
        redis.delete_key(&attachment_data_key(id)).await?;
        return Ok(());
    };
    redis
        .delete_string_and_srem(
            &attachment_data_key(id),
            &session_attachments_key(&attachment.session_id),
            id,
        )
        .await?;
    Ok(())
}
