use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DailyAvailabilityError {
    ObjectRequired,
    InvalidStart,
    InvalidEnd,
    SameTime,
}

pub(crate) fn normalize_daily_availability(
    value: Option<&Value>,
) -> Result<Value, DailyAvailabilityError> {
    let Some(value) = value else {
        return Ok(Value::Null);
    };
    if value.is_null() {
        return Ok(Value::Null);
    }
    let Some(object) = value.as_object() else {
        return Err(DailyAvailabilityError::ObjectRequired);
    };
    if object.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Ok(Value::Null);
    }

    let start_time = object
        .get("start_time")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let end_time = object
        .get("end_time")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    validate_daily_availability_window(start_time, end_time)?;

    Ok(json!({
        "enabled": true,
        "start_time": start_time,
        "end_time": end_time,
    }))
}

pub(crate) fn validate_daily_availability_window(
    start_time: &str,
    end_time: &str,
) -> Result<(), DailyAvailabilityError> {
    let start_minute =
        parse_daily_availability_minute(start_time).ok_or(DailyAvailabilityError::InvalidStart)?;
    let end_minute =
        parse_daily_availability_minute(end_time).ok_or(DailyAvailabilityError::InvalidEnd)?;
    if start_minute == end_minute {
        return Err(DailyAvailabilityError::SameTime);
    }
    Ok(())
}

fn parse_daily_availability_minute(value: &str) -> Option<u16> {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return None;
    }
    let hour = parse_two_digits(bytes[0], bytes[1])?;
    let minute = parse_two_digits(bytes[3], bytes[4])?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

fn parse_two_digits(a: u8, b: u8) -> Option<u16> {
    if !a.is_ascii_digit() || !b.is_ascii_digit() {
        return None;
    }
    Some(u16::from(a - b'0') * 10 + u16::from(b - b'0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_daytime_and_cross_midnight_windows() {
        assert_eq!(validate_daily_availability_window("09:00", "18:00"), Ok(()));
        assert_eq!(validate_daily_availability_window("22:00", "06:00"), Ok(()));
    }

    #[test]
    fn rejects_invalid_and_equal_times() {
        assert_eq!(
            validate_daily_availability_window("9:00", "18:00"),
            Err(DailyAvailabilityError::InvalidStart)
        );
        assert_eq!(
            validate_daily_availability_window("09:00", "09:00"),
            Err(DailyAvailabilityError::SameTime)
        );
    }
}
