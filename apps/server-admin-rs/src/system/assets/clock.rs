use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
    time::Instant,
};

use ::time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc2822};
use axum::http::header;
use serde_json::{Value, json};

use crate::{i18n::Translator, state::AppState, time_utils};

use super::{
    CLOCK_STATUS, EXPECTED_TIME_ZONE, NETWORK_TIME_SOURCES, TIME_DRIFT_THRESHOLD_MS,
    process::run_process_success, runtime::detect_system_timezone,
};

pub(super) async fn cached_clock_status(state: &AppState, translator: &Translator) -> Value {
    if let Some(status) = clock_status_guard().clone() {
        return localize_clock_status(status, translator);
    }
    build_clock_status(state, false, translator).await
}

pub(super) async fn refresh_clock_status(state: &AppState, translator: &Translator) -> Value {
    let status = build_clock_status(state, true, translator).await;
    *clock_status_guard() = Some(status.clone());
    status
}

pub(super) async fn build_clock_status(
    state: &AppState,
    checked: bool,
    translator: &Translator,
) -> Value {
    let system_time_ms = time_utils::now_ms();
    let system_time_zone = detect_system_timezone();
    let timezone_mismatch = system_time_zone.as_deref() != Some(EXPECTED_TIME_ZONE);
    let remote = if checked {
        fetch_network_time(state, translator).await
    } else {
        Ok(None)
    };
    let (network_source, remote_time_ms, last_check_error) = match remote {
        Ok(Some(remote)) => (Some(remote.source), Some(remote.epoch_ms), None),
        Ok(None) => (None, None, None),
        Err(error) => (None, None, Some(error)),
    };
    let drift_ms = remote_time_ms.map(|remote| system_time_ms - remote);
    let time_mismatch = drift_ms.is_some_and(|value| value.abs() > TIME_DRIFT_THRESHOLD_MS);
    let mut status = json!({
        "expectedTimeZone": EXPECTED_TIME_ZONE,
        "systemTimeZone": system_time_zone,
        "checkedAt": if checked { Value::String(time_utils::now_iso()) } else { Value::Null },
        "networkSource": network_source,
        "hasRemoteTime": remote_time_ms.is_some(),
        "lastCheckError": last_check_error,
        "systemTimeMs": system_time_ms,
        "remoteTimeMs": remote_time_ms,
        "systemBeijingTime": format_beijing_time(system_time_ms, translator.locale()),
        "remoteBeijingTime": remote_time_ms.and_then(|value| {
            format_beijing_time(value, translator.locale())
        }),
        "driftMs": drift_ms,
        "driftThresholdMs": TIME_DRIFT_THRESHOLD_MS,
        "timeMismatch": time_mismatch,
        "timezoneMismatch": timezone_mismatch,
        "needsAttention": timezone_mismatch || time_mismatch,
        "issues": [],
        "checking": false,
        "syncInProgress": false,
        "lastSyncAt": Value::Null,
        "lastSyncError": Value::Null,
        "syncSummary": Value::Null
    });
    preserve_clock_sync_metadata(&mut status);
    localize_clock_status(status, translator)
}

pub(super) fn localize_clock_status(mut status: Value, translator: &Translator) -> Value {
    let timezone_mismatch = status
        .get("timezoneMismatch")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let time_mismatch = status
        .get("timeMismatch")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let system_time_zone = status
        .get("systemTimeZone")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| translator.t("server.systemClock.unknown"));
    let drift_ms = status.get("driftMs").and_then(Value::as_i64);

    let mut issues = Vec::new();
    if timezone_mismatch {
        issues.push(json!({
            "code": "timezone_mismatch",
            "title": translator.t("server.systemClock.issues.timezone.title"),
            "message": translator.t_params(
                "server.systemClock.issues.timezone.message",
                &[
                    ("timezone", system_time_zone),
                    ("expected", EXPECTED_TIME_ZONE.to_string())
                ]
            )
        }));
    }
    if time_mismatch && let Some(drift_ms) = drift_ms {
        issues.push(json!({
            "code": "time_mismatch",
            "title": translator.t("server.systemClock.issues.timeMismatch.title"),
            "message": translator.t_params(
                "server.systemClock.issues.timeMismatch.message",
                &[("drift", format_drift(drift_ms, translator))]
            )
        }));
    }

    if let Some(object) = status.as_object_mut() {
        object.insert("issues".to_string(), Value::Array(issues));
    }
    status
}

pub(super) fn format_drift(drift_ms: i64, translator: &Translator) -> String {
    let total_seconds = ((drift_ms.abs() + 500) / 1000).max(1);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes <= 0 {
        return translator.t_params(
            "server.systemClock.duration.seconds",
            &[("seconds", seconds.to_string())],
        );
    }
    if seconds == 0 {
        return translator.t_params(
            "server.systemClock.duration.minutes",
            &[("minutes", minutes.to_string())],
        );
    }
    translator.t_params(
        "server.systemClock.duration.minutesSeconds",
        &[
            ("minutes", minutes.to_string()),
            ("seconds", seconds.to_string()),
        ],
    )
}

struct NetworkTimeResult {
    epoch_ms: i64,
    source: String,
}

async fn fetch_network_time(
    state: &AppState,
    translator: &Translator,
) -> Result<Option<NetworkTimeResult>, String> {
    let mut last_error = translator.t("server.systemClock.networkTimeUnavailable");
    for (label, url) in NETWORK_TIME_SOURCES {
        match fetch_network_time_from_source(state, translator, label, url).await {
            Ok(result) => return Ok(Some(result)),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

async fn fetch_network_time_from_source(
    state: &AppState,
    translator: &Translator,
    label: &str,
    url: &str,
) -> Result<NetworkTimeResult, String> {
    let started = Instant::now();
    let mut date_header = state
        .fallback_client
        .head(url)
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::PRAGMA, "no-cache")
        .send()
        .await
        .ok()
        .and_then(|response| {
            response
                .headers()
                .get(header::DATE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        });

    if date_header.is_none() {
        date_header = state
            .fallback_client
            .get(url)
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::PRAGMA, "no-cache")
            .send()
            .await
            .map_err(|_| {
                translator.t_params(
                    "server.systemClock.sourceFetchFailed",
                    &[("source", label.to_string())],
                )
            })?
            .headers()
            .get(header::DATE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
    }

    let date_header = date_header.ok_or_else(|| {
        translator.t_params(
            "server.systemClock.missingDateHeader",
            &[("source", label.to_string())],
        )
    })?;
    let parsed = OffsetDateTime::parse(&date_header, &Rfc2822).map_err(|_| {
        translator.t_params(
            "server.systemClock.invalidDateHeader",
            &[("source", label.to_string())],
        )
    })?;
    let latency_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i64;
    Ok(NetworkTimeResult {
        epoch_ms: parsed.unix_timestamp() * 1000 + network_latency_compensation_ms(latency_ms),
        source: label.to_string(),
    })
}

pub(super) fn network_latency_compensation_ms(latency_ms: i64) -> i64 {
    latency_ms.max(0).saturating_add(1) / 2
}

pub(super) async fn sync_system_clock(
    state: &AppState,
    translator: &Translator,
) -> Result<(String, Value), String> {
    set_clock_sync_in_progress();
    match sync_system_clock_inner(state, translator).await {
        Ok(result) => Ok(result),
        Err(error) => {
            set_clock_sync_error(error.clone());
            Err(error)
        }
    }
}

pub(super) async fn sync_system_clock_inner(
    state: &AppState,
    translator: &Translator,
) -> Result<(String, Value), String> {
    let before = build_clock_status(state, true, translator).await;
    let mut actions = Vec::new();

    if before.get("systemTimeZone").and_then(Value::as_str) != Some(EXPECTED_TIME_ZONE) {
        actions.push(set_system_timezone(translator)?);
    }

    if let Some(remote_time_ms) = before.get("remoteTimeMs").and_then(Value::as_i64) {
        let checked_at_ms = before
            .get("checkedAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or_else(time_utils::now_ms);
        actions.push(set_system_clock(
            clock_sync_target_epoch_ms(remote_time_ms, checked_at_ms, time_utils::now_ms()),
            translator,
        )?);
    }

    if let Some(message) = enable_network_time_sync(translator) {
        actions.push(message);
    }

    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let mut next = build_clock_status(state, true, translator).await;
    let message = if actions.is_empty() {
        translator.t("server.systemClock.statusRefreshed")
    } else {
        actions.join(&translator.t("server.systemClock.actionSeparator"))
    };
    next["syncInProgress"] = Value::Bool(false);
    next["lastSyncAt"] = json!(time_utils::now_iso());
    next["lastSyncError"] = Value::Null;
    next["syncSummary"] = json!(message.clone());
    *clock_status_guard() = Some(next.clone());
    Ok((message, next))
}

pub(super) fn preserve_clock_sync_metadata(status: &mut Value) {
    let previous = clock_status_guard().clone();
    preserve_clock_sync_metadata_from(status, previous.as_ref());
}

pub(super) fn preserve_clock_sync_metadata_from(status: &mut Value, previous: Option<&Value>) {
    let Some(previous) = previous else {
        return;
    };
    let Some(object) = status.as_object_mut() else {
        return;
    };
    for key in [
        "syncInProgress",
        "lastSyncAt",
        "lastSyncError",
        "syncSummary",
    ] {
        if let Some(value) = previous.get(key) {
            object.insert(key.to_string(), value.clone());
        }
    }
}

pub(super) fn set_clock_sync_in_progress() {
    update_cached_clock_sync_metadata(|status| {
        status["syncInProgress"] = Value::Bool(true);
        status["lastSyncError"] = Value::Null;
    });
}

pub(super) fn set_clock_sync_error(message: String) {
    update_cached_clock_sync_metadata(|status| {
        status["syncInProgress"] = Value::Bool(false);
        status["lastSyncAt"] = json!(time_utils::now_iso());
        status["lastSyncError"] = json!(message);
        status["syncSummary"] = Value::Null;
    });
}

pub(super) fn update_cached_clock_sync_metadata(update: impl FnOnce(&mut Value)) {
    let mut guard = clock_status_guard();
    let mut status = guard.take().unwrap_or_else(initial_clock_status);
    update(&mut status);
    *guard = Some(status);
}

pub(super) fn initial_clock_status() -> Value {
    json!({
        "expectedTimeZone": EXPECTED_TIME_ZONE,
        "systemTimeZone": Value::Null,
        "checkedAt": Value::Null,
        "networkSource": Value::Null,
        "hasRemoteTime": false,
        "lastCheckError": Value::Null,
        "systemTimeMs": Value::Null,
        "remoteTimeMs": Value::Null,
        "systemBeijingTime": Value::Null,
        "remoteBeijingTime": Value::Null,
        "driftMs": Value::Null,
        "driftThresholdMs": TIME_DRIFT_THRESHOLD_MS,
        "timeMismatch": false,
        "timezoneMismatch": false,
        "needsAttention": false,
        "issues": [],
        "checking": false,
        "syncInProgress": false,
        "lastSyncAt": Value::Null,
        "lastSyncError": Value::Null,
        "syncSummary": Value::Null
    })
}

fn clock_status_guard() -> MutexGuard<'static, Option<Value>> {
    clock_status_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

pub(super) fn clock_sync_target_epoch_ms(
    remote_time_ms: i64,
    checked_at_ms: i64,
    now_ms: i64,
) -> i64 {
    remote_time_ms + (now_ms - checked_at_ms).max(0)
}

pub(super) fn set_system_timezone(translator: &Translator) -> Result<String, String> {
    if run_process_success("timedatectl", &["set-timezone", EXPECTED_TIME_ZONE]).is_ok() {
        return Ok(translator.t_params(
            "server.systemClock.timezoneSet",
            &[("timezone", EXPECTED_TIME_ZONE.to_string())],
        ));
    }

    let zoneinfo_path = format!("/usr/share/zoneinfo/{EXPECTED_TIME_ZONE}");
    if !Path::new(&zoneinfo_path).exists() {
        return Err(translator.t_params(
            "server.systemClock.missingZoneinfoFile",
            &[("path", zoneinfo_path)],
        ));
    }
    let _ = fs::remove_file("/etc/localtime");
    match std::os::unix::fs::symlink(&zoneinfo_path, "/etc/localtime") {
        Ok(()) => {}
        Err(_) => fs::copy(&zoneinfo_path, "/etc/localtime")
            .map(|_| ())
            .map_err(|error| error.to_string())?,
    }
    fs::write("/etc/timezone", format!("{EXPECTED_TIME_ZONE}\n"))
        .map_err(|error| error.to_string())?;
    Ok(translator.t_params(
        "server.systemClock.timezoneWritten",
        &[("timezone", EXPECTED_TIME_ZONE.to_string())],
    ))
}

pub(super) fn set_system_clock(
    target_epoch_ms: i64,
    translator: &Translator,
) -> Result<String, String> {
    let target_seconds = target_epoch_ms / 1000;
    run_process_success("date", &["-u", "-s", &format!("@{target_seconds}")])?;
    let _ = run_process_success("hwclock", &["--systohc"]);
    Ok(translator.t("server.systemClock.clockAdjusted"))
}

pub(super) fn enable_network_time_sync(translator: &Translator) -> Option<String> {
    let mut actions = Vec::new();
    if run_process_success("timedatectl", &["set-ntp", "true"]).is_ok() {
        actions.push(translator.t("server.systemClock.ntpEnabled"));
    }
    for service in ["systemd-timesyncd", "chrony", "chronyd", "ntp"] {
        if run_process_success("systemctl", &["restart", service]).is_ok() {
            actions.push(translator.t_params(
                "server.systemClock.serviceRestarted",
                &[("service", service.to_string())],
            ));
            break;
        }
    }
    (!actions.is_empty()).then(|| actions.join(&translator.t("server.systemClock.listSeparator")))
}

pub(super) fn format_beijing_time(epoch_ms: i64, locale: &str) -> Option<String> {
    let seconds = epoch_ms.div_euclid(1000);
    let offset = UtcOffset::from_hms(8, 0, 0).ok()?;
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .map(|value| value.to_offset(offset))
        .map(|value| {
            let year = value.year();
            let month = u8::from(value.month());
            let day = value.day();
            let hour = value.hour();
            let minute = value.minute();
            let second = value.second();
            match locale {
                "en" => {
                    format!("{month:02}/{day:02}/{year:04}, {hour:02}:{minute:02}:{second:02}")
                }
                "ko-KR" => {
                    format!("{year:04}. {month:02}. {day:02}. {hour:02}:{minute:02}:{second:02}")
                }
                "zh-Hant" => {
                    format!(
                        "{year:04}/{month:02}/{day:02}\u{2009}{hour:02}:{minute:02}:{second:02}"
                    )
                }
                _ => format!("{year:04}/{month:02}/{day:02} {hour:02}:{minute:02}:{second:02}"),
            }
        })
}

pub(super) fn clock_status_lock() -> &'static Mutex<Option<Value>> {
    CLOCK_STATUS.get_or_init(|| Mutex::new(None))
}
