use super::*;
use utoipa_axum::routes;

pub(in crate::certificates::acme) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(check_dns))
        .routes(routes!(delete_txt))
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteSelection {
    snapshot: String,
    records: Vec<SelectedValue>,
}
#[derive(Deserialize, ToSchema, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SelectedValue {
    name: String,
    value: String,
}

async fn load(state: &AppState, id: &str, t: &Translator) -> Result<Value, Response> {
    match find_acme_application(state, id).await {
        Ok(Some(app)) => Ok(app),
        Ok(None) => Err(response::error(
            StatusCode::NOT_FOUND,
            t.t("server.acmeRoutes.notFound"),
        )),
        Err(e) => Err(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )),
    }
}

#[utoipa::path(get,path="/api/admin/acme/applications/{id}/preflight",tag="acme",params(("id"=String,Path)),responses((status=200,description="Current authoritative DNS observations")))]
async fn check_dns(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    let t = Translator::from_state(&state).await;
    let app = match load(&state, &id, &t).await {
        Ok(app) => app,
        Err(r) => return r,
    };
    match async {
        let check = inspect(&state, &app, &t).await?;
        remember_snapshot(&state, &check).await?;
        Ok::<_, anyhow::Error>(check.report)
    }
    .await
    {
        Ok(report) => response::ok(json!(report)).into_response(),
        Err(e) => response::error(StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
    }
}

#[utoipa::path(post,path="/api/admin/acme/applications/{id}/preflight/delete",tag="acme",params(("id"=String,Path)),request_body=DeleteSelection,responses((status=200,description="Per-value deletion outcomes and refreshed DNS observations")))]
async fn delete_txt(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(selection): Json<DeleteSelection>,
) -> Response {
    let t = Translator::from_state(&state).await;
    if selection.records.is_empty()
        || selection.records.len() > 32
        || selection.snapshot.len() != 32
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            acme_route_text(&t, "invalidRequestBody"),
        );
    }
    let lock_id = uuid::Uuid::new_v4().to_string();
    let lock = with_runtime_lock_lease(
        json!({"locked":true,"lockId":lock_id,"applicationId":id,"startedAt":now_node_iso()}),
    );
    match state
        .storage
        .store
        .set_json_value_nx_ex(ACME_RUNTIME_LOCK_KEY, &lock, 300)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return response::error(
                StatusCode::CONFLICT,
                t.t("server.acmeJobRunner.activeTaskRunning"),
            );
        }
        Err(e) => return response::error(StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
    }
    // Once deletion is submitted, finish rechecking and release the lease
    // even if the caller navigates away while waiting for the HTTP response.
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let worker = state.clone();
    let worker_t = t.clone();
    state.spawn_background("acme-delete-txt", async move {
        let result = tokio_time::timeout(std::time::Duration::from_secs(120), async {
            let app = find_acme_application(&worker, &id)
                .await?
                .ok_or_else(|| anyhow::anyhow!(worker_t.t("server.acmeRoutes.notFound")))?;
            delete_selected(&worker, &app, selection, &worker_t).await
        })
        .await;
        if let Err(error) = release_acme_runtime_lock(&worker, &lock).await {
            tracing::warn!(%error,"could not release DNS cleanup lock");
        }
        let _ = sender.send(result);
    });
    let Ok(result) = receiver.await else {
        return response::error(
            StatusCode::SERVICE_UNAVAILABLE,
            acme_route_text(&t, "dnsCleanupTimeout"),
        );
    };
    match result {
        Ok(Ok(value)) => response::ok(value).into_response(),
        Ok(Err(e)) => gate_error(e),
        Err(_) => response::error(
            StatusCode::GATEWAY_TIMEOUT,
            acme_route_text(&t, "dnsCleanupTimeout"),
        ),
    }
}

async fn delete_selected(
    state: &AppState,
    app: &Value,
    selection: DeleteSelection,
    t: &Translator,
) -> anyhow::Result<Value> {
    let saved = state
        .storage
        .store
        .get_json_value(&format!(
            "fn_knock:acme:dns-snapshot:{}",
            selection.snapshot
        ))
        .await?;
    let before = inspect(state, app, t).await?;
    let Some(saved) = saved.filter(|s| s["signature"] == before.signature) else {
        remember(state, &before).await?;
        return Err(NeedsReview(before.report).into());
    };
    if dns_cooling(state, app).await? {
        return Err(NeedsReview(before.report).into());
    }
    let approved: Vec<TxtRecord> = serde_json::from_value(saved["records"].clone())?;
    let provider = TxtProvider::new(
        t,
        app["dnsType"].as_str().unwrap_or_default(),
        &app["credentials"],
    )?;
    let mut results = vec![];
    for selected in selection.records.into_iter().collect::<BTreeSet<_>>() {
        let record = approved
            .iter()
            .find(|r| r.name == selected.name && r.value == selected.value);
        let group = before
            .report
            .groups
            .iter()
            .find(|g| g.name == selected.name);
        let _configuration_lock = state.gateway.acme_output_lock.lock().await;
        let current = find_acme_application(state, app["id"].as_str().unwrap_or_default()).await?;
        let matches = current.is_some_and(|latest| config_signature(&latest) == before.signature);
        let limited = dns_cooling(state, app).await?;
        let outcome = match (record, group) {
            _ if !matches => "changed",
            _ if limited => "failed",
            (Some(record), Some(group))
                if group.zone == record.zone
                    && matches!(
                        group.status.as_str(),
                        "clear" | "existing" | "unsafe" | "propagating"
                    ) =>
            {
                match provider
                    .delete_selected(
                        record,
                        before.records.contains(record)
                            && group.selectable.contains(&selected.value),
                    )
                    .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        if let Some(limit) =
                            error.downcast_ref::<crate::ddns::acme_txt::ProviderLimit>()
                        {
                            persist_provider_limit(state, app, limit).await?;
                        }
                        "failed"
                    }
                }
            }
            _ => "changed",
        };
        if matches!(outcome, "deleted" | "absent") {
            record_deletion(state, &before.signature, &selected.name, &selected.value).await?;
        }
        tracing::info!(application_id=%app["id"],name=%selected.name,outcome,"ACME TXT cleanup");
        results.push(json!({"name":selected.name,"value":selected.value,"outcome":outcome}));
    }
    let latest = find_acme_application(state, app["id"].as_str().unwrap_or_default())
        .await?
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeRoutes.notFound")))?;
    let after = inspect(state, &latest, t).await?;
    remember(state, &after).await?;
    Ok(json!({"results":results,"report":after.report}))
}
