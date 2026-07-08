use std::time::SystemTime;

use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub fn now_ms() -> i64 {
    let now = OffsetDateTime::now_utc();
    now.unix_timestamp() * 1000 + i64::from(now.millisecond())
}

pub fn now_seconds() -> i64 {
    now_ms().div_euclid(1000)
}

pub fn now_iso() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

pub fn system_time_iso(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    OffsetDateTime::from_unix_timestamp(duration.as_secs() as i64)
        .ok()
        .and_then(|time| time.format(&Rfc3339).ok())
        .unwrap_or_else(now_iso)
}

pub fn iso_after_seconds(seconds: i64) -> String {
    (OffsetDateTime::now_utc() + Duration::seconds(seconds))
        .format(&Rfc3339)
        .unwrap_or_else(|_| now_iso())
}

pub fn node_iso_now() -> String {
    format_node_iso_timestamp(OffsetDateTime::now_utc())
}

pub fn node_iso_after_seconds(seconds: i64) -> String {
    format_node_iso_timestamp(OffsetDateTime::now_utc() + Duration::seconds(seconds))
}

pub fn normalize_node_iso(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let timestamp = OffsetDateTime::parse(trimmed, &Rfc3339)
        .ok()?
        .to_offset(UtcOffset::UTC);
    Some(format_node_iso_timestamp(timestamp))
}

pub fn iso_from_ms(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000) as i128;
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|time| {
            (time + Duration::milliseconds(millis as i64))
                .format(&Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

pub fn parse_iso_ms(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|time| time.unix_timestamp() * 1000 + i64::from(time.millisecond()))
}

pub fn local_date_from_ms(ms: i64) -> String {
    let timestamp = ms.div_euclid(1000);
    let Ok(utc) = OffsetDateTime::from_unix_timestamp(timestamp) else {
        return "1970-01-01".to_string();
    };
    let local = time::UtcOffset::current_local_offset()
        .map(|offset| utc.to_offset(offset))
        .unwrap_or(utc);
    format!(
        "{:04}-{:02}-{:02}",
        local.year(),
        u8::from(local.month()),
        local.day()
    )
}

fn format_node_iso_timestamp(timestamp: OffsetDateTime) -> String {
    let timestamp = timestamp.to_offset(UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        timestamp.year(),
        u8::from(timestamp.month()),
        timestamp.day(),
        timestamp.hour(),
        timestamp.minute(),
        timestamp.second(),
        timestamp.millisecond()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_time_iso_formats_epoch_seconds_like_existing_helpers() {
        assert_eq!(
            system_time_iso(SystemTime::UNIX_EPOCH),
            "1970-01-01T00:00:00Z"
        );
        assert_eq!(
            system_time_iso(SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_234)),
            "1970-01-01T00:00:01Z"
        );
    }

    #[test]
    fn normalize_node_iso_preserves_node_millisecond_shape() {
        assert_eq!(
            normalize_node_iso("2026-07-07T10:18:23.946511792Z"),
            Some("2026-07-07T10:18:23.946Z".to_string())
        );
        assert_eq!(
            normalize_node_iso("2026-07-07T18:18:23+08:00"),
            Some("2026-07-07T10:18:23.000Z".to_string())
        );
        assert_eq!(normalize_node_iso("not-a-date"), None);
    }
}
