use super::*;

impl ConnectionManager {
    /// Read auth metadata without TTL cleanup or primary writer admission.
    /// Capture time after admission so queued reads cannot revive expired keys.
    pub(crate) async fn get_auth_live_strings(
        &self,
        keys: Vec<String>,
    ) -> RedisResult<Vec<Option<String>>> {
        self.call_auth_read(move |conn| {
            let tx = conn.transaction()?;
            let now = crate::time_utils::now_ms();
            let values = {
                let mut statement = tx.prepare_cached(
                    "SELECT strings.value FROM kv_strings AS strings
                     JOIN kv_keys AS keys ON keys.key = strings.key
                     WHERE strings.key = ?1 AND keys.kind = 'string'
                       AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)",
                )?;
                keys.iter()
                    .map(|key| {
                        statement
                            .query_row(params![key, now], |row| row.get(0))
                            .optional()
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            tx.commit()?;
            Ok(values)
        })
        .await
    }

    pub(crate) async fn get_auth_live_hash_field(
        &self,
        key: String,
        field: String,
    ) -> RedisResult<Option<String>> {
        self.call_auth_read(move |conn| {
            conn.query_row(
                "SELECT hashes.value FROM kv_hash AS hashes
                 JOIN kv_keys AS keys ON keys.key = hashes.key
                 WHERE hashes.key = ?1 AND hashes.field = ?2 AND keys.kind = 'hash'
                   AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?3)",
                params![key, field, crate::time_utils::now_ms()],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
        })
        .await
    }
}
