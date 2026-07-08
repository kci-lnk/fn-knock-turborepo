use super::*;

pub(super) async fn get_config(State(state): State<AppState>) -> Response {
    match ssh_security_details(&state).await {
        Ok(details) => response::ok(details).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load SSH security config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "loadConfigFailed"),
            )
        }
    }
}

pub(super) async fn update_config(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match update_ssh_security_config(&state, body, &translator).await {
        Ok(details) => response::ok(details).into_response(),
        Err(SshError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(SshError::Runtime(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(SshError::Storage(error)) => {
            tracing::warn!(%error, "failed to update SSH security config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "updateConfigFailed"),
            )
        }
    }
}

pub(super) async fn sync_firewall(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match sync_firewall_blocks_now(&state, &translator).await {
        Ok(value) => {
            let ports = value
                .get("ports")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_i64)
                        .map(|port| port.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let message = ssh_security_route_text_params(
                &translator,
                "syncFirewallSuccess",
                &[
                    (
                        "allowedCidrs",
                        value
                            .get("allowed_cidrs")
                            .and_then(Value::as_i64)
                            .unwrap_or(0)
                            .to_string(),
                    ),
                    ("ports", ports),
                    (
                        "synced",
                        value
                            .get("synced")
                            .and_then(Value::as_i64)
                            .unwrap_or(0)
                            .to_string(),
                    ),
                ],
            );
            Json(json!({ "success": true, "message": message, "data": value })).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to sync SSH firewall");
            let message = error.to_string();
            response::error(
                StatusCode::BAD_GATEWAY,
                if message.trim().is_empty() {
                    ssh_security_route_text(&translator, "syncFirewallFailed")
                } else {
                    message
                },
            )
        }
    }
}

pub(super) async fn clear_firewall(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let availability = ssh_security_availability(&state, &translator);
    if !availability.available {
        return response::error(StatusCode::BAD_REQUEST, availability.reason);
    }
    let payload = json!({
        "chain_name": SSH_FIREWALL_CHAIN,
        "parent_chain": ["INPUT", "DOCKER-USER"]
    });
    match state.go_backend.clear_ssh_firewall(&payload).await {
        Ok(value) => {
            if let Err(error) = ensure_go_success(value, &translator, "clearSshPolicyFailed")
                .map_err(|error| {
                    tracing::warn!(%error, "go backend rejected SSH firewall clear");
                    error
                })
            {
                return response::error(
                    StatusCode::BAD_GATEWAY,
                    if error.to_string().trim().is_empty() {
                        ssh_security_route_text(&translator, "clearFirewallFailed")
                    } else {
                        error.to_string()
                    },
                );
            }
            let mut cleared = 0usize;
            match active_blocks(&state).await {
                Ok(records) => {
                    for record in records {
                        if let Some(ip) = record.get("ip").and_then(Value::as_str)
                            && mark_block_removed(&state, ip, "manual")
                                .await
                                .unwrap_or(false)
                        {
                            if let Err(error) = clear_failures(&state, ip).await {
                                tracing::warn!(%error, ip, "failed to clear SSH failures");
                            }
                            cleared += 1;
                        }
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to mark SSH blocks as cleared"),
            }
            Json(json!({
                "success": true,
                "message": ssh_security_route_text(&translator, "clearFirewallSuccess"),
                "data": { "cleared_blocks": cleared }
            }))
            .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "go backend SSH firewall clear failed");
            response::error(
                StatusCode::BAD_GATEWAY,
                ssh_security_route_text(&translator, "clearFirewallFailed"),
            )
        }
    }
}

pub(super) async fn login_logs(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Response {
    let page = parse_positive(query.page.as_deref(), 1, i64::MAX);
    let limit = parse_positive(query.limit.as_deref(), 20, 100);
    let search = query.search.unwrap_or_default().trim().to_ascii_lowercase();
    let outcome = query.outcome.unwrap_or_default();
    let outcome = if outcome == "success" || outcome == "failure" {
        outcome
    } else {
        String::new()
    };
    let mut entries = query_recent_ssh_logs((page * limit * 5 + limit * 5).max(500) as usize);
    entries.retain(|entry| {
        if !outcome.is_empty()
            && entry.get("outcome").and_then(Value::as_str) != Some(outcome.as_str())
        {
            return false;
        }
        if search.is_empty() {
            return true;
        }
        ["ip", "username", "raw"].iter().any(|key| {
            entry
                .get(*key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains(&search)
        })
    });
    entries.sort_by(|left, right| {
        iso_score(right.get("happened_at").and_then(Value::as_str))
            .cmp(&iso_score(left.get("happened_at").and_then(Value::as_str)))
    });
    entries = coalesce_success_login_logs(entries);
    let total = entries.len();
    let start = ((page - 1) * limit) as usize;
    let mut items = entries
        .into_iter()
        .skip(start)
        .take(limit as usize)
        .collect::<Vec<_>>();
    hydrate_ip_location_records(&state, &mut items, |entry| {
        entry
            .get("id")
            .and_then(Value::as_str)
            .map(|id| format!("ssh-login-log|{id}"))
    })
    .await;
    response::ok(json!({ "items": items, "total": total, "page": page, "limit": limit }))
        .into_response()
}

pub(super) async fn list_blocks(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let page = parse_positive(query.page.as_deref(), 1, i64::MAX);
    let limit = parse_positive(query.limit.as_deref(), 20, 100);
    let search = query.search.unwrap_or_default().trim().to_ascii_lowercase();
    match list_active_blocks(&state, page, limit, &search).await {
        Ok((mut items, total)) => {
            hydrate_ip_location_records(&state, &mut items, |record| {
                record
                    .get("ip")
                    .and_then(Value::as_str)
                    .map(|ip| format!("ssh-blocklist|{ip}"))
            })
            .await;
            response::ok(json!({ "items": items, "total": total, "page": page, "limit": limit }))
                .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to list SSH security blocks");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "listBlocksFailed"),
            )
        }
    }
}

pub(super) async fn get_block(
    State(state): State<AppState>,
    AxumPath(ip): AxumPath<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let normalized = normalize_ip(&ip);
    if normalized.is_empty() {
        return response::error(
            StatusCode::NOT_FOUND,
            ssh_security_route_text(&translator, "blockNotFound"),
        );
    }
    match load_block(&state, &normalized).await {
        Ok(Some(mut record)) if is_active_block(&record, time_utils::now_ms()) => {
            hydrate_ip_location_records(&state, std::slice::from_mut(&mut record), |record| {
                record
                    .get("ip")
                    .and_then(Value::as_str)
                    .map(|ip| format!("ssh-blocklist|{ip}"))
            })
            .await;
            response::ok(record).into_response()
        }
        Ok(_) => response::error(
            StatusCode::NOT_FOUND,
            ssh_security_route_text(&translator, "blockNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, ip = normalized, "failed to load SSH block");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "loadBlockFailed"),
            )
        }
    }
}

pub(super) async fn delete_block(
    State(state): State<AppState>,
    AxumPath(ip): AxumPath<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match remove_block(&state, &ip, "manual", &translator).await {
        Ok(true) => response::success_empty().into_response(),
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            ssh_security_route_text(&translator, "blockNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to remove SSH block");
            response::error(
                StatusCode::BAD_REQUEST,
                ssh_security_route_text(&translator, "removeBlockFailed"),
            )
        }
    }
}

pub(super) async fn delete_blocks(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed = parse_json_body(&body);
    let raw_ips = parsed
        .get("ips")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(delete_ip_value_to_string)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if raw_ips.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            ssh_security_route_text(&translator, "selectIps"),
        );
    }
    let mut removed = 0usize;
    let mut seen = HashSet::new();
    for raw_ip in raw_ips {
        let ip = normalize_ip(&raw_ip);
        if ip.is_empty() || !seen.insert(ip.clone()) {
            continue;
        }
        match remove_block(&state, &ip, "manual", &translator).await {
            Ok(true) => removed += 1,
            Ok(false) => {}
            Err(error) => tracing::warn!(%error, ip, "failed to remove SSH block"),
        }
    }
    response::ok(json!({ "removed": removed })).into_response()
}
