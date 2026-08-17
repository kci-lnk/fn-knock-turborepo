use super::*;

impl Store {
    pub async fn list_traffic_points(
        &self,
        user_id: &str,
        direction: &str,
        from_sec: i64,
        to_sec: i64,
        host: Option<&str>,
    ) -> crate::storage::StorageResult<Vec<TrafficDeltaPoint>> {
        let key = traffic_key(user_id, direction, host);
        let mut conn = self.conn();
        let members: Vec<String> = conn.zrangebyscore_analytics(key, from_sec, to_sec).await?;
        Ok(parse_traffic_points(&members))
    }

    pub async fn list_error5xx_points(
        &self,
        user_id: &str,
        from_sec: i64,
        to_sec: i64,
        host: Option<&str>,
    ) -> crate::storage::StorageResult<Vec<TrafficDeltaPoint>> {
        let key = error5xx_key(user_id, host);
        let mut conn = self.conn();
        let members: Vec<String> = conn.zrangebyscore_analytics(key, from_sec, to_sec).await?;
        Ok(parse_traffic_points(&members))
    }

    pub async fn record_traffic_snapshot(
        &self,
        user_id: &str,
        records: &[TrafficSnapshotRecord],
        now_sec: i64,
        keep_seconds: i64,
    ) -> crate::storage::StorageResult<(f64, f64, f64)> {
        if records.is_empty() {
            return Ok((0.0, 0.0, 0.0));
        }

        let keep_seconds = keep_seconds.clamp(60, 365 * 24 * 3600);
        let expire_before_sec = now_sec - keep_seconds;
        let mut last_keys = Vec::with_capacity(records.len() * 3);
        for record in records {
            last_keys.push(traffic_last_total_key(
                user_id,
                "in",
                record.host.as_deref(),
            ));
            last_keys.push(traffic_last_total_key(
                user_id,
                "out",
                record.host.as_deref(),
            ));
            last_keys.push(error5xx_last_total_key(user_id, record.host.as_deref()));
        }

        let mut conn = self.conn();
        let last_values: Vec<Option<String>> = redis::cmd("MGET")
            .arg(last_keys)
            .query_async(&mut conn)
            .await?;

        let mut pipe = redis::pipe();
        let mut global_delta_in = 0.0;
        let mut global_delta_out = 0.0;
        let mut global_delta_5xx = 0.0;

        for (index, record) in records.iter().enumerate() {
            let offset = index * 3;
            let last_in = last_values.get(offset).and_then(parse_finite);
            let last_out = last_values.get(offset + 1).and_then(parse_finite);
            let last_5xx = last_values.get(offset + 2).and_then(parse_finite);
            let delta_in = compute_counter_delta(record.total_in, last_in);
            let delta_out = compute_counter_delta(record.total_out, last_out);
            let delta_5xx = compute_counter_delta(record.error_5xx, last_5xx);

            if record.host.is_none() {
                global_delta_in = delta_in;
                global_delta_out = delta_out;
                global_delta_5xx = delta_5xx;
            }

            let key_in = traffic_key(user_id, "in", record.host.as_deref());
            let key_out = traffic_key(user_id, "out", record.host.as_deref());
            let key_5xx = error5xx_key(user_id, record.host.as_deref());

            pipe.set(
                traffic_last_total_key(user_id, "in", record.host.as_deref()),
                finite_number_string(record.total_in),
            )
            .ignore();
            pipe.set(
                traffic_last_total_key(user_id, "out", record.host.as_deref()),
                finite_number_string(record.total_out),
            )
            .ignore();
            pipe.set(
                error5xx_last_total_key(user_id, record.host.as_deref()),
                finite_number_string(record.error_5xx),
            )
            .ignore();

            pipe.zadd(&key_in, traffic_member(now_sec, delta_in), now_sec)
                .ignore();
            pipe.zadd(&key_out, traffic_member(now_sec, delta_out), now_sec)
                .ignore();
            pipe.zadd(&key_5xx, traffic_member(now_sec, delta_5xx), now_sec)
                .ignore();
            pipe.sadd(TRAFFIC_KEY_INDEX, &key_in).ignore();
            pipe.sadd(TRAFFIC_KEY_INDEX, &key_out).ignore();
            pipe.sadd(ERROR5XX_KEY_INDEX, &key_5xx).ignore();
            pipe.zrembyscore(&key_in, 0, expire_before_sec).ignore();
            pipe.zrembyscore(&key_out, 0, expire_before_sec).ignore();
            pipe.zrembyscore(&key_5xx, 0, expire_before_sec).ignore();
        }

        let _: () = pipe.query_async(&mut conn).await?;
        Ok((global_delta_in, global_delta_out, global_delta_5xx))
    }

    pub async fn cleanup_traffic_metrics(
        &self,
        keep_seconds: i64,
    ) -> crate::storage::StorageResult<usize> {
        let keep_seconds = keep_seconds.clamp(60, 365 * 24 * 3600);
        let expire_before_sec = chrono_like_now_seconds() - keep_seconds;
        let mut conn = self.conn();
        let traffic_keys: Vec<String> = conn.smembers(TRAFFIC_KEY_INDEX).await?;
        let error_keys: Vec<String> = conn.smembers(ERROR5XX_KEY_INDEX).await?;
        let keys = traffic_keys
            .iter()
            .chain(error_keys.iter())
            .filter(|key| !key.trim().is_empty())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return Ok(0);
        }
        let mut pipe = redis::pipe();
        for key in &keys {
            pipe.zrembyscore(key, 0, expire_before_sec).ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        self.cleanup_empty_traffic_metric_keys(&traffic_keys, TRAFFIC_KEY_INDEX)
            .await?;
        self.cleanup_empty_traffic_metric_keys(&error_keys, ERROR5XX_KEY_INDEX)
            .await?;
        Ok(keys.len())
    }

    async fn cleanup_empty_traffic_metric_keys(
        &self,
        keys: &[String],
        index_key: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        for chunk in keys
            .iter()
            .filter(|key| !key.trim().is_empty())
            .collect::<Vec<_>>()
            .chunks(100)
        {
            let mut empty_keys = Vec::new();
            for key in chunk {
                let count: i64 = conn.zcard(key.as_str()).await?;
                if count == 0 {
                    empty_keys.push((*key).clone());
                }
            }
            if empty_keys.is_empty() {
                continue;
            }
            let mut pipe = redis::pipe();
            pipe.srem(index_key, empty_keys.clone()).ignore();
            for key in &empty_keys {
                pipe.del(key).ignore();
                if let Some(last_key) = traffic_last_total_key_for_metric_key(key) {
                    pipe.del(last_key).ignore();
                }
            }
            let _: () = pipe.query_async(&mut conn).await?;
        }
        Ok(())
    }
}

pub(super) fn traffic_last_total_key_for_metric_key(key: &str) -> Option<String> {
    let key = key.trim();
    if let Some(rest) = key.strip_prefix("fn_knock:traffic:")
        && (rest.ends_with(":in") || rest.ends_with(":out"))
    {
        return Some(format!("fn_knock:traffic:last:{rest}"));
    }
    if let Some(rest) = key.strip_prefix("fn_knock:errors:")
        && rest.ends_with(":5xx")
    {
        return Some(format!("fn_knock:errors:last:{rest}"));
    }
    None
}
