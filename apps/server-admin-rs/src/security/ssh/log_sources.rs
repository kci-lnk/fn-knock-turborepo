use super::*;

pub(super) fn query_recent_ssh_logs(limit: usize) -> Vec<Value> {
    if command_available("journalctl") {
        let output = Command::new("journalctl")
            .args([
                "-u",
                "ssh",
                "-u",
                "sshd",
                "-n",
                &limit.to_string(),
                "-o",
                "json",
            ])
            .output();
        if let Ok(output) = output
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            let entries = text
                .lines()
                .filter_map(parse_journal_line)
                .collect::<Vec<_>>();
            if !entries.is_empty() {
                return entries;
            }
        }
    }
    query_auth_log(limit)
}

pub(super) fn parse_journal_line(line: &str) -> Option<Value> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let message = value.get("MESSAGE").and_then(Value::as_str)?;
    let micros = value
        .get("__REALTIME_TIMESTAMP")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    let happened_at = if micros > 0 {
        millis_to_iso(micros / 1000)
    } else {
        time_utils::now_iso()
    };
    parse_ssh_message(message, &happened_at, "journal")
}

pub(super) fn query_auth_log(limit: usize) -> Vec<Value> {
    let mut entries = Vec::new();
    for path in AUTH_LOG_CANDIDATES {
        let Ok(lines) = read_log_lines(path) else {
            continue;
        };
        for line in lines.into_iter().rev() {
            if entries.len() >= limit {
                break;
            }
            if !line.to_ascii_lowercase().contains("sshd") {
                continue;
            }
            if let Some((happened_at, message)) = parse_syslog_line(&line)
                && let Some(entry) = parse_ssh_message(&message, &happened_at, "auth.log")
            {
                entries.push(entry);
            }
        }
        if !entries.is_empty() {
            break;
        }
    }
    entries
}

pub(super) fn read_log_lines(path: &str) -> std::io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let reader: Box<dyn Read> = if path.ends_with(".gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut lines = BufReader::new(reader)
        .lines()
        .map_while(Result::ok)
        .collect::<Vec<_>>();
    if lines.len() > 5000 {
        lines = lines.split_off(lines.len() - 5000);
    }
    Ok(lines)
}

pub(super) fn parse_ssh_message(message: &str, happened_at: &str, source: &str) -> Option<Value> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    let lower = message.to_ascii_lowercase();
    let (outcome, invalid_user, marker) = if lower.contains("accepted ") {
        ("success", false, " for ")
    } else if lower.contains("failed ") && lower.contains(" for invalid user ") {
        ("failure", true, " for invalid user ")
    } else if lower.contains("failed ") {
        ("failure", false, " for ")
    } else {
        return None;
    };
    let ip = extract_between(message, " from ", " port ").and_then(|value| {
        let ip = normalize_ip(value);
        if ip.is_empty() { None } else { Some(ip) }
    })?;
    let port = extract_after(message, " port ")
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|port| *port > 0 && *port <= 65535);
    let auth_method = message
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
        .filter(|value| !value.is_empty());
    let username = extract_between(message, marker, " from ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let id = fingerprint(&[source, happened_at, outcome, username, &ip, message].join("|"));
    let mut entry = json!({
        "id": id,
        "happened_at": happened_at,
        "outcome": outcome,
        "username": username,
        "invalid_user": invalid_user,
        "ip": ip,
        "service": "sshd",
        "source": source,
        "raw": message
    });
    if let Some(port) = port {
        entry["port"] = json!(port);
    }
    if let Some(auth_method) = auth_method {
        entry["auth_method"] = json!(auth_method);
    }
    Some(entry)
}

pub(super) fn parse_syslog_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let month = parts.next()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    let time_text = parts.next()?;
    let _host = parts.next()?;
    let message = parts.collect::<Vec<_>>().join(" ");
    let month = match month {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let mut t = time_text.split(':');
    let hour = t.next()?.parse::<u8>().ok()?;
    let minute = t.next()?.parse::<u8>().ok()?;
    let second = t.next()?.parse::<u8>().ok()?;
    let now = time::OffsetDateTime::now_utc();
    let date =
        time::Date::from_calendar_date(now.year(), time::Month::try_from(month).ok()?, day).ok()?;
    let time_value = time::Time::from_hms(hour, minute, second).ok()?;
    let mut happened_at = date.with_time(time_value).assume_utc();
    if happened_at > now + time::Duration::days(1) {
        happened_at = happened_at.replace_year(now.year() - 1).ok()?;
    }
    Some((
        happened_at
            .format(&time::format_description::well_known::Rfc3339)
            .ok()?,
        message,
    ))
}

pub(super) fn detect_log_source() -> &'static str {
    if command_available("journalctl") {
        return "journal";
    }
    if AUTH_LOG_CANDIDATES
        .iter()
        .any(|path| Path::new(path).exists())
    {
        return "auth.log";
    }
    "unavailable"
}
