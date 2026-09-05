use super::*;

pub(super) fn import_success_response(
    data: Value,
    from_directory: bool,
    translator: &Translator,
) -> Response {
    let has_warnings = data
        .get("warnings")
        .and_then(Value::as_array)
        .is_some_and(|warnings| !warnings.is_empty());
    let key = match (from_directory, has_warnings) {
        (true, true) => "importFnosSuccessWithWarnings",
        (true, false) => "importFnosSuccess",
        (false, true) => "importSuccessWithWarnings",
        (false, false) => "importSuccess",
    };
    axum::Json(json!({
        "success": true,
        "message": admin_backup_text(translator, key),
        "data": data
    }))
    .into_response()
}

pub(super) fn admin_backup_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.backup.{key}"))
}

pub(super) fn maintenance_backup_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.maintenanceBackup.{key}"))
}

pub(super) fn maintenance_clear_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.maintenanceClear.{key}"))
}

pub(super) fn maintenance_backup_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.maintenanceBackup.{key}"), params)
}

pub(super) fn backup_import_version_range() -> String {
    format!("{APP_BACKUP_IMPORT_MIN_VERSION} ~ {APP_LOCAL_VERSION}")
}

pub(super) fn backup_error_key_message(key: &str, params: &[(&str, String)]) -> String {
    let mut params_object = serde_json::Map::new();
    for (key, value) in params {
        params_object.insert((*key).to_string(), Value::String(value.clone()));
    }
    json!({
        MAINTENANCE_BACKUP_ERROR_MARKER: true,
        "kind": "key",
        "key": key,
        "params": params_object,
    })
    .to_string()
}

#[cfg(test)]
pub(super) fn backup_command_error_message(
    message: String,
    code: i32,
    detail: Option<String>,
) -> String {
    json!({
        MAINTENANCE_BACKUP_ERROR_MARKER: true,
        "kind": "command_error",
        "message": message,
        "code": code,
        "detail": detail.unwrap_or_default(),
    })
    .to_string()
}

pub(super) fn localize_backup_error_message(translator: &Translator, message: &str) -> String {
    let normalized = message.trim();
    if let Some(localized) = localize_structured_backup_error(translator, normalized) {
        return localized;
    }
    if let Some(localized) = localize_backup_entry_error(translator, normalized) {
        return localized;
    }
    match normalized {
        "Backup operation is busy" => match translator.locale() {
            "zh-CN" => "备份任务正在进行，请稍后重试".to_string(),
            "zh-TW" => "備份工作正在進行，請稍後重試".to_string(),
            _ => "Another backup operation is in progress; retry shortly".to_string(),
        },
        "Backup service is shutting down" => match translator.locale() {
            "zh-CN" => "备份服务正在关闭，请稍后重试".to_string(),
            "zh-TW" => "備份服務正在關閉，請稍後重試".to_string(),
            _ => "The backup service is shutting down; retry shortly".to_string(),
        },
        "Backup share directory is not configured" => {
            maintenance_backup_text(translator, "shareDirectoryMissing")
        }
        "Automatic backup interval is invalid" => {
            maintenance_backup_text(translator, "automaticIntervalInvalid")
        }
        "Automatic backup retention is invalid" => {
            maintenance_backup_text(translator, "automaticRetentionInvalid")
        }
        "Invalid backup path" => maintenance_backup_text(translator, "invalidBackupPath"),
        "Backup file not found" => {
            maintenance_backup_text(translator, "directoryImportFileNotFound")
        }
        "Backup file cannot be read" => {
            maintenance_backup_text(translator, "directoryImportFileUnreadable")
        }
        "Backup path must be a file" => {
            maintenance_backup_text(translator, "directoryImportFileOnly")
        }
        "Backup archive content is required" => {
            maintenance_backup_text(translator, "archiveContentMissing")
        }
        "Backup archive base64 is invalid" => {
            maintenance_backup_text(translator, "archiveBase64Invalid")
        }
        "Backup archive is empty" => maintenance_backup_text(translator, "archiveEmpty"),
        "Backup archive is too large" => maintenance_backup_text(translator, "archiveTooLarge"),
        "Backup export is too large" => maintenance_backup_text(translator, "exportTooLarge"),
        "Backup directory import archive is too large" => {
            maintenance_backup_text(translator, "directoryImportTooLarge")
        }
        "Backup archive password is invalid" => {
            maintenance_backup_text(translator, "archivePasswordInvalid")
        }
        "Backup payload is not valid UTF-8" => {
            maintenance_backup_text(translator, "payloadUtf8Invalid")
        }
        "Backup JSON payload is invalid" => maintenance_backup_text(translator, "jsonParseFailed"),
        "Backup payload must be an object" => {
            maintenance_backup_text(translator, "payloadObjectInvalid")
        }
        "Backup app version is missing" => maintenance_backup_text(translator, "missingAppVersion"),
        "Backup exported_at is missing" => maintenance_backup_text(translator, "missingExportedAt"),
        "Backup entries are missing" => maintenance_backup_text(translator, "missingEntries"),
        "Backup contains duplicated Redis keys" => {
            maintenance_backup_text(translator, "duplicateRedisKey")
        }
        _ if normalized.starts_with("Backup archive filename must end with") => {
            maintenance_backup_text_params(
                translator,
                "invalidBackupExtension",
                &[("extension", KNOCK_BACKUP_EXTENSION.to_string())],
            )
        }
        _ if normalized.starts_with("Backup archive file must end with") => {
            maintenance_backup_text_params(
                translator,
                "directoryImportExtensionOnly",
                &[("extension", KNOCK_BACKUP_EXTENSION.to_string())],
            )
        }
        _ if normalized.starts_with("Backup archive is missing ") => {
            maintenance_backup_text_params(
                translator,
                "archiveMissingPayload",
                &[("filename", KNOCK_BACKUP_JSON_FILENAME.to_string())],
            )
        }
        _ if normalized.starts_with("Unsupported Redis type for backup: ") => {
            let detail = normalized
                .strip_prefix("Unsupported Redis type for backup: ")
                .unwrap_or_default();
            let (data_type, key) = parse_type_and_key_detail(detail);
            maintenance_backup_text_params(
                translator,
                "unsupportedRedisExportType",
                &[("type", data_type), ("key", key)],
            )
        }
        _ if normalized.starts_with("Unsupported backup schema version.") => {
            maintenance_backup_text_params(
                translator,
                "unsupportedSchemaVersion",
                &[("version", APP_BACKUP_SCHEMA_VERSION.to_string())],
            )
        }
        _ if normalized.starts_with("Unsupported backup prefix.") => {
            maintenance_backup_text_params(
                translator,
                "unsupportedPrefix",
                &[("prefix", KNOCK_BACKUP_PREFIX.to_string())],
            )
        }
        _ if normalized.starts_with("Backup app version ")
            && normalized.contains(" is unsupported.") =>
        {
            let app_version = normalized
                .strip_prefix("Backup app version ")
                .and_then(|rest| rest.split_once(" is unsupported."))
                .map(|(version, _)| version.trim().to_string())
                .filter(|version| !version.is_empty())
                .unwrap_or_else(|| "unknown".to_string());
            maintenance_backup_text_params(
                translator,
                "appVersionUnsupported",
                &[
                    ("currentVersion", APP_LOCAL_VERSION.to_string()),
                    ("range", backup_import_version_range()),
                    ("appVersion", app_version),
                ],
            )
        }
        _ if normalized.starts_with("Failed to read backup archive") => {
            maintenance_backup_text(translator, "readArchiveFailed")
        }
        _ => normalized.to_string(),
    }
}

pub(super) fn localize_structured_backup_error(
    translator: &Translator,
    message: &str,
) -> Option<String> {
    let parsed: Value = serde_json::from_str(message).ok()?;
    let object = parsed.as_object()?;
    if object
        .get(MAINTENANCE_BACKUP_ERROR_MARKER)
        .and_then(Value::as_bool)
        != Some(true)
    {
        return None;
    }

    match object.get("kind").and_then(Value::as_str) {
        Some("key") => {
            let key = object.get("key").and_then(Value::as_str)?;
            let params_object = object
                .get("params")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let params = params_object
                .iter()
                .map(|(key, value)| (key.as_str(), backup_param_value_to_string(value)))
                .collect::<Vec<_>>();
            Some(maintenance_backup_text_params(translator, key, &params))
        }
        Some("command_error") => {
            let raw_message = object.get("message").and_then(Value::as_str).unwrap_or("");
            let message = if raw_message.trim().is_empty() {
                maintenance_backup_text(translator, "unknownError")
            } else {
                localize_backup_error_message(translator, raw_message)
            };
            let code = object
                .get("code")
                .and_then(Value::as_i64)
                .unwrap_or(-1)
                .to_string();
            let detail = object
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if detail.is_empty() {
                Some(maintenance_backup_text_params(
                    translator,
                    "commandError",
                    &[("message", message), ("code", code)],
                ))
            } else {
                Some(maintenance_backup_text_params(
                    translator,
                    "commandErrorWithDetail",
                    &[("message", message), ("code", code), ("detail", detail)],
                ))
            }
        }
        _ => None,
    }
}

pub(super) fn backup_param_value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

pub(super) fn localize_backup_entry_error(
    translator: &Translator,
    message: &str,
) -> Option<String> {
    let rest = message.strip_prefix("entries[")?;
    let (index, suffix) = rest.split_once(']')?;
    if index.is_empty() {
        return None;
    }
    let value_label = format!("entries[{index}].value");
    match suffix {
        " must be an object" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryObjectRequired",
                &[("index", index.to_string())],
            ));
        }
        ".type is unsupported" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryTypeUnsupported",
                &[("index", index.to_string())],
            ));
        }
        ".ttl_ms is invalid" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryTtlInvalid",
                &[("index", index.to_string())],
            ));
        }
        ".value must be a string" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryValueStringRequired",
                &[("index", index.to_string())],
            ));
        }
        ".value must be an object" => {
            return Some(maintenance_backup_text_params(
                translator,
                "objectRequired",
                &[("label", value_label)],
            ));
        }
        ".value must be an array" => {
            return Some(maintenance_backup_text_params(
                translator,
                "arrayRequired",
                &[("label", value_label)],
            ));
        }
        ".value must contain only strings" => {
            return Some(maintenance_backup_text_params(
                translator,
                "stringArrayOnlyStrings",
                &[("label", value_label)],
            ));
        }
        _ => {}
    }

    if suffix.starts_with(".key must start with ") {
        return Some(maintenance_backup_text_params(
            translator,
            "entryKeyPrefixRequired",
            &[
                ("index", index.to_string()),
                ("prefix", KNOCK_BACKUP_PREFIX.to_string()),
            ],
        ));
    }

    if let Some(field) = suffix
        .strip_prefix(".value.")
        .and_then(|rest| rest.strip_suffix(" must be a string"))
    {
        return Some(maintenance_backup_text_params(
            translator,
            "fieldStringRequired",
            &[("label", value_label), ("field", field.to_string())],
        ));
    }

    let rest = suffix.strip_prefix(".value[")?;
    let (item_index, item_suffix) = rest.split_once(']')?;
    if item_index.is_empty() {
        return None;
    }
    match item_suffix {
        ".member must be a string" => Some(maintenance_backup_text_params(
            translator,
            "zsetMemberRequired",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        ".score is invalid" => Some(maintenance_backup_text_params(
            translator,
            "zsetScoreRequired",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        ".id must be a string" => Some(maintenance_backup_text_params(
            translator,
            "streamIdRequired",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        ".fields must be an array" => Some(maintenance_backup_text_params(
            translator,
            "stringArrayRequired",
            &[(
                "label",
                format!("entries[{index}].value[{item_index}].fields"),
            )],
        )),
        ".fields must contain only strings" => Some(maintenance_backup_text_params(
            translator,
            "stringArrayOnlyStrings",
            &[(
                "label",
                format!("entries[{index}].value[{item_index}].fields"),
            )],
        )),
        ".fields is invalid" => Some(maintenance_backup_text_params(
            translator,
            "streamFieldsInvalid",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        _ => None,
    }
}

pub(super) fn parse_type_and_key_detail(detail: &str) -> (String, String) {
    let Some((data_type, rest)) = detail.trim().split_once(" (") else {
        return (detail.trim().to_string(), "unknown".to_string());
    };
    (
        data_type.trim().to_string(),
        rest.trim_end_matches(')').trim().to_string(),
    )
}
