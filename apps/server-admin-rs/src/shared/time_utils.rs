use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

pub fn now_ms() -> i64 {
    let now = OffsetDateTime::now_utc();
    now.unix_timestamp() * 1000 + i64::from(now.millisecond())
}

pub fn now_iso() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

pub fn iso_after_seconds(seconds: i64) -> String {
    (OffsetDateTime::now_utc() + Duration::seconds(seconds))
        .format(&Rfc3339)
        .unwrap_or_else(|_| now_iso())
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
