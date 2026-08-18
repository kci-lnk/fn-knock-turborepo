use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::OwnedMutexGuard;
use url::Url;
use uuid::Uuid;

use crate::state::AppState;

use super::{
    adapters::{AdapterRegistry, ApplyCheckpoint, provider_descriptors},
    credentials::CredentialStore,
    model::*,
    projection,
    repository::Repository,
};

#[derive(Debug)]
pub enum ServiceError {
    NotFound,
    Validation(String),
    Conflict(String),
    Failed(String),
}

impl ServiceError {
    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "面板连接不存在".to_string(),
            Self::Validation(value) | Self::Conflict(value) | Self::Failed(value) => value.clone(),
        }
    }
}

pub fn providers() -> Vec<ProviderDescriptor> {
    provider_descriptors()
}

pub async fn connections(state: &AppState) -> Result<Vec<PanelConnection>, ServiceError> {
    let repository = Repository::new(state);
    let mut output = Vec::new();
    for mut connection in repository
        .connections()
        .await
        .map_err(ServiceError::Failed)?
    {
        connection.credential_configured = credential_store(state).configured(&connection.id);
        output.push(
            repository
                .decorate(connection)
                .await
                .map_err(ServiceError::Failed)?,
        );
    }
    Ok(output)
}

pub async fn create(
    state: &AppState,
    input: ConnectionInput,
) -> Result<PanelConnection, ServiceError> {
    validate_credential_update(input.clear_credential, input.credential.as_deref())?;
    validate_common(
        &input.name,
        &input.base_url,
        input.api_path.as_deref(),
        &input.grouping,
        &input.auto_sync,
    )?;
    let _guard = state.panel_sync.config_lock.lock().await;
    let now = now();
    let id = Uuid::new_v4().to_string();
    let mut connection = PanelConnection {
        id: id.clone(),
        name: input.name.trim().to_string(),
        provider: input.provider,
        base_url: normalized_base_url(&input.base_url)?,
        api_path: normalize_api_path(input.provider, input.api_path.as_deref())?,
        allow_invalid_tls: input.allow_invalid_tls,
        grouping: normalize_grouping(input.grouping)?,
        auto_sync: input.auto_sync,
        credential_configured: false,
        verified_at: None,
        verified_version: None,
        created_at: now.clone(),
        updated_at: now,
        last_run: None,
        next_sync_at: None,
    };
    if let Some(secret) = non_empty_secret(input.credential) {
        credential_store(state)
            .write(&id, &secret)
            .map_err(ServiceError::Failed)?;
        connection.credential_configured = true;
    }
    let repository = Repository::new(state);
    let mut all = repository
        .connections()
        .await
        .map_err(ServiceError::Failed)?;
    all.push(connection.clone());
    if let Err(error) = repository.save_connections(&all).await {
        let _ = credential_store(state).delete(&id);
        return Err(ServiceError::Failed(error));
    }
    Ok(connection)
}

pub async fn update(
    state: &AppState,
    id: &str,
    input: ConnectionUpdateInput,
) -> Result<PanelConnection, ServiceError> {
    validate_credential_update(input.clear_credential, input.credential.as_deref())?;
    validate_common(
        &input.name,
        &input.base_url,
        input.api_path.as_deref(),
        &input.grouping,
        &input.auto_sync,
    )?;
    let _connection_guard = state
        .panel_sync
        .connection_lock(id)
        .await
        .try_lock_owned()
        .map_err(|_| ServiceError::Conflict("该连接已有同步任务正在运行".to_string()))?;
    let _guard = state.panel_sync.config_lock.lock().await;
    let repository = Repository::new(state);
    let mut all = repository
        .connections()
        .await
        .map_err(ServiceError::Failed)?;
    let Some(index) = all.iter().position(|item| item.id == id) else {
        return Err(ServiceError::NotFound);
    };
    let previous_all = all.clone();
    let previous = all[index].clone();
    let previous_secret = credential_store(state)
        .read(id)
        .map_err(ServiceError::Failed)?;
    let supplied_secret = non_empty_secret(input.credential.clone());
    let credential_changed = input.clear_credential || supplied_secret.is_some();
    let base_url = normalized_base_url(&input.base_url)?;
    let api_path = normalize_api_path(previous.provider, input.api_path.as_deref())?;
    let endpoint_changed = previous.base_url != base_url
        || previous.api_path != api_path
        || previous.allow_invalid_tls != input.allow_invalid_tls;
    let will_have_credential =
        supplied_secret.is_some() || (!input.clear_credential && previous_secret.is_some());
    let mut connection = previous.clone();
    connection.name = input.name.trim().to_string();
    connection.base_url = base_url;
    connection.api_path = api_path;
    connection.allow_invalid_tls = input.allow_invalid_tls;
    connection.grouping = normalize_grouping(input.grouping)?;
    connection.auto_sync = input.auto_sync;
    connection.updated_at = now();
    if endpoint_changed || credential_changed {
        connection.verified_at = None;
        connection.verified_version = None;
    }
    connection.credential_configured = will_have_credential;
    all[index] = connection.clone();
    repository
        .save_connections(&all)
        .await
        .map_err(ServiceError::Failed)?;
    let credential_result = if input.clear_credential {
        credential_store(state).delete(id)
    } else if let Some(secret) = supplied_secret {
        credential_store(state).write(id, &secret)
    } else {
        Ok(())
    };
    if let Err(error) = credential_result {
        let config_rollback = repository.save_connections(&previous_all).await.err();
        let credential_rollback = restore_credential(state, id, previous_secret.as_deref()).err();
        return Err(ServiceError::Failed(with_rollback_errors(
            error,
            config_rollback,
            credential_rollback,
        )));
    }
    if endpoint_changed && let Err(error) = repository.clear_managed(id).await {
        let config_rollback = repository.save_connections(&previous_all).await.err();
        let credential_rollback = restore_credential(state, id, previous_secret.as_deref()).err();
        return Err(ServiceError::Failed(with_rollback_errors(
            error,
            config_rollback,
            credential_rollback,
        )));
    }
    state.panel_sync.source_changed.notify_one();
    Ok(connection)
}

pub async fn delete(
    state: &AppState,
    id: &str,
    request: DeleteConnectionRequest,
) -> Result<(), ServiceError> {
    let _connection_guard = state
        .panel_sync
        .connection_lock(id)
        .await
        .try_lock_owned()
        .map_err(|_| ServiceError::Conflict("该连接已有同步任务正在运行".to_string()))?;
    if request.cleanup_remote {
        let plan = preview(state, id, true).await?;
        let source_revision = request.source_revision.as_deref().ok_or_else(|| {
            ServiceError::Validation("清理远端内容前必须先生成清理预览".to_string())
        })?;
        let plan_hash = request.plan_hash.as_deref().ok_or_else(|| {
            ServiceError::Validation("清理远端内容前必须先生成清理预览".to_string())
        })?;
        if source_revision != plan.preview.source_revision || plan_hash != plan.preview.plan_hash {
            return Err(ServiceError::Conflict(
                "源配置或远端状态已变化，请重新预览清理内容".to_string(),
            ));
        }
        let connection = Repository::new(state)
            .connection(id)
            .await
            .map_err(ServiceError::Failed)?
            .ok_or(ServiceError::NotFound)?;
        if !AdapterRegistry::resolve(connection.provider)
            .capabilities()
            .can_delete
        {
            return Err(ServiceError::Validation(
                "该面板不支持安全清理远端内容".to_string(),
            ));
        }
        let credential = credential_store(state)
            .read(id)
            .map_err(ServiceError::Failed)?
            .ok_or_else(|| ServiceError::Validation("连接未配置凭据".to_string()))?;
        let _permit = state
            .panel_sync
            .concurrency
            .acquire()
            .await
            .map_err(|_| ServiceError::Failed("同步调度器已关闭".to_string()))?;
        let checkpoint = ApplyCheckpoint::new(plan.managed.clone());
        if let Err(error) = AdapterRegistry::resolve(connection.provider)
            .apply(
                &AdapterContext {
                    connection,
                    credential,
                },
                &plan,
                &checkpoint,
            )
            .await
        {
            Repository::new(state)
                .save_managed(id, &checkpoint.latest())
                .await
                .map_err(ServiceError::Failed)?;
            return Err(ServiceError::Failed(error));
        }
    }
    detach(state, id).await
}

async fn detach(state: &AppState, id: &str) -> Result<(), ServiceError> {
    let _guard = state.panel_sync.config_lock.lock().await;
    let repository = Repository::new(state);
    let mut all = repository
        .connections()
        .await
        .map_err(ServiceError::Failed)?;
    let before = all.len();
    all.retain(|item| item.id != id);
    if all.len() == before {
        return Err(ServiceError::NotFound);
    }
    let previous_managed = repository.managed(id).await.map_err(ServiceError::Failed)?;
    let previous_secret = credential_store(state)
        .read(id)
        .map_err(ServiceError::Failed)?;
    repository
        .clear_managed(id)
        .await
        .map_err(ServiceError::Failed)?;
    if let Err(error) = credential_store(state).delete(id) {
        let rollback = repository.save_managed(id, &previous_managed).await;
        return Err(ServiceError::Failed(with_rollback_error(
            error,
            rollback.err(),
        )));
    }
    if let Err(error) = repository.save_connections(&all).await {
        let managed_rollback = repository.save_managed(id, &previous_managed).await.err();
        let credential_rollback = restore_credential(state, id, previous_secret.as_deref()).err();
        return Err(ServiceError::Failed(with_rollback_errors(
            error,
            managed_rollback,
            credential_rollback,
        )));
    }
    state.panel_sync.forget_connection(id).await;
    Ok(())
}

pub async fn test(
    state: &AppState,
    input: TestConnectionInput,
) -> Result<ProbeResult, ServiceError> {
    let testing_saved_configuration = input.draft.is_none();
    let saved = match input.connection_id.as_deref() {
        Some(id) => Repository::new(state)
            .connection(id)
            .await
            .map_err(ServiceError::Failed)?
            .ok_or(ServiceError::NotFound)
            .map(Some)?,
        None => None,
    };
    let (connection, supplied_secret, may_use_saved_secret) = if let Some(draft) = input.draft {
        validate_credential_update(draft.clear_credential, draft.credential.as_deref())?;
        validate_common(
            &draft.name,
            &draft.base_url,
            draft.api_path.as_deref(),
            &draft.grouping,
            &draft.auto_sync,
        )?;
        let id = saved
            .as_ref()
            .map(|item| item.id.clone())
            .unwrap_or_else(|| "draft".to_string());
        (
            PanelConnection {
                id,
                name: draft.name,
                provider: draft.provider,
                base_url: normalized_base_url(&draft.base_url)?,
                api_path: normalize_api_path(draft.provider, draft.api_path.as_deref())?,
                allow_invalid_tls: draft.allow_invalid_tls,
                grouping: normalize_grouping(draft.grouping)?,
                auto_sync: draft.auto_sync,
                credential_configured: false,
                verified_at: None,
                verified_version: None,
                created_at: now(),
                updated_at: now(),
                last_run: None,
                next_sync_at: None,
            },
            non_empty_secret(draft.credential),
            !draft.clear_credential,
        )
    } else {
        (
            saved
                .clone()
                .ok_or_else(|| ServiceError::Validation("必须提供草稿或连接 ID".to_string()))?,
            None,
            true,
        )
    };
    let saved_secret = if supplied_secret.is_none() && may_use_saved_secret {
        match &saved {
            Some(item) => credential_store(state)
                .read(&item.id)
                .map_err(ServiceError::Failed)?,
            None => None,
        }
    } else {
        None
    };
    let credential = supplied_secret
        .or(saved_secret)
        .ok_or_else(|| ServiceError::Validation("请填写 API 凭据".to_string()))?;
    let context = AdapterContext {
        connection: connection.clone(),
        credential,
    };
    let result = AdapterRegistry::resolve(connection.provider)
        .probe(&context)
        .await
        .map_err(ServiceError::Failed)?;
    if testing_saved_configuration {
        mark_verified(
            state,
            &connection.id,
            &connection.updated_at,
            result.version.clone(),
        )
        .await?;
    }
    Ok(result)
}

pub async fn preview(
    state: &AppState,
    id: &str,
    cleanup_remote: bool,
) -> Result<AdapterPlan, ServiceError> {
    let repository = Repository::new(state);
    let connection = repository
        .connection(id)
        .await
        .map_err(ServiceError::Failed)?
        .ok_or(ServiceError::NotFound)?;
    ensure_ready(state, &connection)?;
    let credential = credential_store(state)
        .read(id)
        .map_err(ServiceError::Failed)?
        .ok_or_else(|| ServiceError::Validation("连接未配置凭据".to_string()))?;
    let context = AdapterContext {
        connection: connection.clone(),
        credential,
    };
    let adapter = AdapterRegistry::resolve(connection.provider);
    let managed = repository.managed(id).await.map_err(ServiceError::Failed)?;
    let config = state
        .storage
        .store
        .get_config()
        .await
        .map_err(|error| ServiceError::Failed(error.to_string()))?;
    let missing_sync_ids = projection::eligible_mappings_missing_sync_id(&config);
    if missing_sync_ids > 0 {
        return Err(ServiceError::Failed(format!(
            "有 {missing_sync_ids} 条可同步映射缺少稳定 sync_id；主机映射身份迁移尚未完成"
        )));
    }
    let mut projected = projection::project(&config, &connection.grouping);
    let capabilities = adapter.capabilities();
    if cleanup_remote {
        if !capabilities.can_delete {
            return Err(ServiceError::Validation(
                "该面板不支持安全清理远端内容；只能解除连接并报告残留".to_string(),
            ));
        }
        projected.groups.clear();
        projected.links.clear();
        projected
            .warnings
            .push("此预览会删除当前连接已登记的远端分类和链接，然后解除连接".to_string());
    }
    if !capabilities.supports_icon {
        let omitted = projected
            .links
            .iter()
            .filter(|link| link.icon.is_some())
            .count();
        for link in &mut projected.links {
            link.icon = None;
        }
        if omitted > 0 {
            projected.warnings.push(format!(
                "{} 不支持稳定的图标写入，已省略 {omitted} 个图标",
                connection.provider.label()
            ));
        }
    }
    let remote = adapter
        .inspect(&context, &managed, &projected)
        .await
        .map_err(ServiceError::Failed)?;
    Ok(adapter.plan(&connection, projected, managed, remote))
}

pub async fn enqueue_manual(
    state: &AppState,
    id: &str,
    request: SyncRequest,
) -> Result<SyncAccepted, ServiceError> {
    let guard = state
        .panel_sync
        .connection_lock(id)
        .await
        .try_lock_owned()
        .map_err(|_| ServiceError::Conflict("该连接已有同步任务正在运行".to_string()))?;
    let generation = state.panel_sync.generation();
    let plan = preview(state, id, false).await?;
    if state.panel_sync.generation() != generation {
        return Err(ServiceError::Conflict(
            "面板同步配置已重置，请重新预览".to_string(),
        ));
    }
    if !plan.preview.can_apply {
        return Err(ServiceError::Conflict(
            "远端存在所有权冲突，请先处理冲突后重新预览".to_string(),
        ));
    }
    if plan.preview.source_revision != request.source_revision
        || plan.preview.plan_hash != request.plan_hash
    {
        return Err(ServiceError::Conflict(
            "源配置或远端状态已变化，请重新预览".to_string(),
        ));
    }
    let connection = Repository::new(state)
        .connection(id)
        .await
        .map_err(ServiceError::Failed)?
        .ok_or(ServiceError::NotFound)?;
    let credential = credential_store(state)
        .read(id)
        .map_err(ServiceError::Failed)?
        .ok_or_else(|| ServiceError::Validation("连接未配置凭据".to_string()))?;
    let run = queued_run(id, RunTrigger::Manual, &plan.preview);
    Repository::new(state)
        .save_run(&run)
        .await
        .map_err(ServiceError::Failed)?;
    let run_id = run.id.clone();
    let state_for_task = state.clone();
    state.spawn_background("panel-sync-manual", async move {
        run_apply(
            state_for_task,
            AdapterContext {
                connection,
                credential,
            },
            plan,
            run,
            guard,
            generation,
        )
        .await;
    });
    Ok(SyncAccepted { run_id })
}

pub async fn enqueue_automatic(state: AppState, connection: PanelConnection, trigger: RunTrigger) {
    let connection_id = connection.id;
    let Ok(guard) = state
        .panel_sync
        .connection_lock(&connection_id)
        .await
        .try_lock_owned()
    else {
        return;
    };
    let connection = match Repository::new(&state).connection(&connection_id).await {
        Ok(Some(connection))
            if connection.auto_sync.enabled && connection.verified_at.is_some() =>
        {
            connection
        }
        _ => return,
    };
    let generation = state.panel_sync.generation();
    let plan = match preview(&state, &connection_id, false).await {
        Ok(plan) => plan,
        Err(error) => {
            record_automatic_failure(
                &state,
                &connection_id,
                trigger,
                RunStatus::Failed,
                None,
                error.message(),
            )
            .await;
            return;
        }
    };
    if state.panel_sync.generation() != generation {
        return;
    }
    if !plan.preview.can_apply {
        record_automatic_failure(
            &state,
            &connection_id,
            trigger,
            RunStatus::Conflict,
            Some(&plan.preview),
            "远端存在所有权冲突，请先处理冲突".to_string(),
        )
        .await;
        return;
    }
    let credential = match credential_store(&state).read(&connection_id) {
        Ok(Some(credential)) => credential,
        Ok(None) => {
            record_automatic_failure(
                &state,
                &connection_id,
                trigger,
                RunStatus::Failed,
                Some(&plan.preview),
                "连接未配置凭据".to_string(),
            )
            .await;
            return;
        }
        Err(error) => {
            record_automatic_failure(
                &state,
                &connection_id,
                trigger,
                RunStatus::Failed,
                Some(&plan.preview),
                error,
            )
            .await;
            return;
        }
    };
    let run = queued_run(&connection_id, trigger, &plan.preview);
    if Repository::new(&state).save_run(&run).await.is_err() {
        return;
    }
    run_apply(
        state,
        AdapterContext {
            connection,
            credential,
        },
        plan,
        run,
        guard,
        generation,
    )
    .await;
}

async fn run_apply(
    state: AppState,
    context: AdapterContext,
    plan: AdapterPlan,
    mut run: SyncRun,
    _connection_guard: OwnedMutexGuard<()>,
    generation: u64,
) {
    if state.panel_sync.generation() != generation {
        run.status = RunStatus::Failed;
        run.message = Some("面板同步配置已重置，任务已取消".to_string());
        run.finished_at = Some(now());
        let _ = Repository::new(&state).save_run(&run).await;
        return;
    }
    let _permit = tokio::select! {
        _ = state.shutdown.cancelled() => {
            run.status = RunStatus::Failed;
            run.message = Some("服务关闭，同步任务已取消".to_string());
            run.finished_at = Some(now());
            let _ = Repository::new(&state).save_run(&run).await;
            return;
        }
        permit = state.panel_sync.concurrency.acquire() => {
            let Ok(permit) = permit else { return; };
            permit
        }
    };
    run.status = RunStatus::Running;
    let _ = Repository::new(&state).save_run(&run).await;
    let previous_failed = Repository::new(&state)
        .runs(&context.connection.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .skip(1)
        .find(|previous| previous.finished_at.is_some())
        .is_some_and(|previous| matches!(previous.status, RunStatus::Failed | RunStatus::Conflict));
    if plan.preview.counts.create == 0
        && plan.preview.counts.update == 0
        && plan.preview.counts.delete == 0
    {
        run.status = RunStatus::Skipped;
        run.message = Some(if plan.preview.counts.residual > 0 {
            "源投影无变化；远端残留已保留".to_string()
        } else {
            "源投影与远端状态均无变化".to_string()
        });
        run.finished_at = Some(now());
        if run.trigger != RunTrigger::Manual
            && previous_failed
            && let Err(error) = crate::system_events::publish_panel_sync_event(
                &state,
                &context.connection.id,
                true,
                true,
                run.message.as_deref(),
            )
            .await
        {
            tracing::warn!(%error, "failed to publish panel sync recovery event");
        }
        let _ = Repository::new(&state).save_run(&run).await;
        return;
    }
    let checkpoint = ApplyCheckpoint::new(plan.managed.clone());
    let invalidated = state.panel_sync.runs_invalidated.notified();
    tokio::pin!(invalidated);
    if state.panel_sync.generation() != generation {
        run.status = RunStatus::Failed;
        run.message = Some("面板同步配置已重置，任务已取消".to_string());
        run.finished_at = Some(now());
        let _ = Repository::new(&state).save_run(&run).await;
        return;
    }
    let result = tokio::select! {
        _ = state.shutdown.cancelled() => Err("服务关闭，同步任务已取消".to_string()),
        _ = &mut invalidated => Err("面板同步配置已重置，任务已取消".to_string()),
        result = AdapterRegistry::resolve(context.connection.provider)
            .apply(&context, &plan, &checkpoint) => result,
    };
    run.finished_at = Some(now());
    if state.panel_sync.generation() != generation {
        run.status = RunStatus::Failed;
        run.message = Some("面板同步配置已重置，已丢弃本次运行的本地状态".to_string());
        let _ = Repository::new(&state).save_run(&run).await;
        return;
    }
    match result {
        Ok(managed) => {
            if let Err(error) = Repository::new(&state)
                .save_managed(&context.connection.id, &managed)
                .await
            {
                run.status = RunStatus::Failed;
                run.message = Some(error);
            } else {
                run.status = RunStatus::Success;
                run.message = Some("同步完成".to_string());
            }
        }
        Err(error) => {
            run.status = RunStatus::Failed;
            let checkpoint_error = Repository::new(&state)
                .save_managed(&context.connection.id, &checkpoint.latest())
                .await
                .err();
            run.message = Some(with_rollback_error(error, checkpoint_error));
        }
    }
    if run.trigger != RunTrigger::Manual {
        let success = run.status == RunStatus::Success;
        if (!success || previous_failed)
            && let Err(error) = crate::system_events::publish_panel_sync_event(
                &state,
                &context.connection.id,
                success,
                success && previous_failed,
                run.message.as_deref(),
            )
            .await
        {
            tracing::warn!(%error, "failed to publish panel sync event");
        }
    }
    let _ = Repository::new(&state).save_run(&run).await;
}

async fn record_automatic_failure(
    state: &AppState,
    connection_id: &str,
    trigger: RunTrigger,
    status: RunStatus,
    preview: Option<&SyncPreview>,
    message: String,
) {
    let mut run = if let Some(preview) = preview {
        queued_run(connection_id, trigger, preview)
    } else {
        SyncRun {
            id: Uuid::new_v4().to_string(),
            connection_id: connection_id.to_string(),
            trigger,
            status: RunStatus::Queued,
            source_revision: String::new(),
            plan_hash: String::new(),
            counts: PlanCounts::default(),
            warnings: Vec::new(),
            message: None,
            started_at: now(),
            finished_at: None,
        }
    };
    run.status = status;
    run.message = Some(message);
    run.finished_at = Some(now());
    if let Err(error) = Repository::new(state).save_run(&run).await {
        tracing::warn!(%error, "failed to save automatic panel sync failure");
    }
    if let Err(error) = crate::system_events::publish_panel_sync_event(
        state,
        connection_id,
        false,
        false,
        run.message.as_deref(),
    )
    .await
    {
        tracing::warn!(%error, "failed to publish automatic panel sync failure");
    }
}

fn ensure_ready(state: &AppState, connection: &PanelConnection) -> Result<(), ServiceError> {
    if connection.verified_at.is_none() {
        return Err(ServiceError::Validation("连接尚未验证".to_string()));
    }
    if !credential_store(state).configured(&connection.id) {
        return Err(ServiceError::Validation("连接未配置凭据".to_string()));
    }
    Ok(())
}

async fn mark_verified(
    state: &AppState,
    id: &str,
    tested_updated_at: &str,
    version: Option<String>,
) -> Result<(), ServiceError> {
    let _guard = state.panel_sync.config_lock.lock().await;
    let repository = Repository::new(state);
    let mut all = repository
        .connections()
        .await
        .map_err(ServiceError::Failed)?;
    let Some(connection) = all.iter_mut().find(|item| item.id == id) else {
        return Err(ServiceError::NotFound);
    };
    if connection.updated_at != tested_updated_at {
        return Err(ServiceError::Conflict(
            "连接配置在测试期间发生变化，请重新测试".to_string(),
        ));
    }
    connection.verified_at = Some(now());
    connection.verified_version = version;
    connection.updated_at = now();
    repository
        .save_connections(&all)
        .await
        .map_err(ServiceError::Failed)
}

fn validate_common(
    name: &str,
    base_url: &str,
    api_path: Option<&str>,
    grouping: &GroupingConfig,
    auto: &AutoSyncConfig,
) -> Result<(), ServiceError> {
    if name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(ServiceError::Validation(
            "连接名称不能为空且最多 80 个字符".to_string(),
        ));
    }
    if base_url.chars().count() > 2048 {
        return Err(ServiceError::Validation(
            "Base URL 最多 2048 个字符".to_string(),
        ));
    }
    normalized_base_url(base_url)?;
    if let Some(path) = api_path
        && !path.trim().is_empty()
        && (!path.trim().starts_with('/')
            || path.chars().count() > 512
            || path.chars().any(char::is_control))
    {
        return Err(ServiceError::Validation(
            "API 路径必须以 / 开头".to_string(),
        ));
    }
    if !(5..=1440).contains(&auto.interval_minutes) {
        return Err(ServiceError::Validation(
            "自动同步周期必须为 5 至 1440 分钟".to_string(),
        ));
    }
    if grouping.namespace.trim().is_empty()
        || grouping.namespace.chars().count() > 80
        || grouping.namespace.chars().any(char::is_control)
        || grouping.single_group_name.chars().count() > 80
        || grouping.single_group_name.chars().any(char::is_control)
    {
        return Err(ServiceError::Validation(
            "命名空间不能为空；命名空间和分类名不得包含控制字符且最多 80 个字符".to_string(),
        ));
    }
    Ok(())
}

fn normalized_base_url(value: &str) -> Result<String, ServiceError> {
    let url = Url::parse(value.trim())
        .map_err(|_| ServiceError::Validation("Base URL 无效".to_string()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(ServiceError::Validation(
            "Base URL 仅支持 HTTP/HTTPS 且必须包含主机".to_string(),
        ));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
    {
        return Err(ServiceError::Validation(
            "Base URL 不允许包含凭据、查询参数或片段".to_string(),
        ));
    }
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn validate_credential_update(clear: bool, value: Option<&str>) -> Result<(), ServiceError> {
    if value.is_some_and(|value| value.len() > 16 * 1024) {
        return Err(ServiceError::Validation("API 凭据最多 16 KiB".to_string()));
    }
    if clear && value.is_some_and(|value| !value.trim().is_empty()) {
        return Err(ServiceError::Validation(
            "清除凭据与填写新凭据不能同时进行".to_string(),
        ));
    }
    Ok(())
}

fn normalize_api_path(
    provider: PanelProvider,
    value: Option<&str>,
) -> Result<String, ServiceError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(provider.default_api_path());
    if !value.starts_with('/') || value.contains('#') || value.contains("//") {
        return Err(ServiceError::Validation("API 路径无效".to_string()));
    }
    let value = value.trim_end_matches('/');
    Ok(if value.is_empty() { "/" } else { value }.to_string())
}

fn normalize_grouping(mut value: GroupingConfig) -> Result<GroupingConfig, ServiceError> {
    value.namespace = value.namespace.trim().to_string();
    value.single_group_name = value.single_group_name.trim().to_string();
    if value.mode == GroupMode::Single && value.single_group_name.is_empty() {
        value.single_group_name = value.namespace.clone();
    }
    Ok(value)
}

fn credential_store(state: &AppState) -> CredentialStore {
    CredentialStore::new(&state.settings.data_dir)
}
fn restore_credential(state: &AppState, id: &str, secret: Option<&str>) -> Result<(), String> {
    if let Some(secret) = secret {
        credential_store(state).write(id, secret)
    } else {
        credential_store(state).delete(id)
    }
}
fn with_rollback_error(error: String, rollback: Option<String>) -> String {
    rollback.map_or(error.clone(), |rollback| {
        format!("{error}；本地状态恢复失败: {rollback}")
    })
}
fn with_rollback_errors(error: String, first: Option<String>, second: Option<String>) -> String {
    let details = [first, second].into_iter().flatten().collect::<Vec<_>>();
    if details.is_empty() {
        error
    } else {
        format!("{error}；本地状态恢复失败: {}", details.join("；"))
    }
}
fn non_empty_secret(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}
fn queued_run(id: &str, trigger: RunTrigger, preview: &SyncPreview) -> SyncRun {
    SyncRun {
        id: Uuid::new_v4().to_string(),
        connection_id: id.to_string(),
        trigger,
        status: RunStatus::Queued,
        source_revision: preview.source_revision.clone(),
        plan_hash: preview.plan_hash.clone(),
        counts: preview.counts.clone(),
        warnings: preview.warnings.clone(),
        message: None,
        started_at: now(),
        finished_at: None,
    }
}

pub async fn clear_credentials_after_backup_restore(state: &AppState) -> Result<(), String> {
    let _guard = state.panel_sync.config_lock.lock().await;
    state.panel_sync.invalidate_runs();
    credential_store(state).clear_all()?;
    let repository = Repository::new(state);
    let mut all = repository.connections().await?;
    for connection in &mut all {
        connection.credential_configured = false;
        connection.verified_at = None;
        connection.verified_version = None;
        connection.auto_sync.enabled = false;
    }
    repository.save_connections(&all).await
}

pub fn clear_all_credentials(state: &AppState) -> Result<(), String> {
    state.panel_sync.invalidate_runs();
    credential_store(state).clear_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.runtime_target = "linux".to_string();
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url.clear();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "panel-sync-test-token".to_string();
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

    fn saved_connection(updated_at: &str) -> PanelConnection {
        PanelConnection {
            id: "11111111-1111-4111-8111-111111111111".to_string(),
            name: "NAS".to_string(),
            provider: PanelProvider::OneNav,
            base_url: "https://nav.example.test".to_string(),
            api_path: "/index.php?c=api".to_string(),
            allow_invalid_tls: false,
            grouping: GroupingConfig::default(),
            auto_sync: AutoSyncConfig::default(),
            credential_configured: true,
            verified_at: None,
            verified_version: None,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            last_run: None,
            next_sync_at: None,
        }
    }

    #[tokio::test]
    async fn a_stale_probe_cannot_verify_a_newer_connection_configuration() {
        let (_directory, state) = test_state().await;
        let repository = Repository::new(&state);
        repository
            .save_connections(&[saved_connection("before-test")])
            .await
            .unwrap();
        repository
            .save_connections(&[saved_connection("changed-during-test")])
            .await
            .unwrap();

        let result = mark_verified(
            &state,
            "11111111-1111-4111-8111-111111111111",
            "before-test",
            None,
        )
        .await;
        assert!(matches!(result, Err(ServiceError::Conflict(_))));
        assert!(
            repository.connections().await.unwrap()[0]
                .verified_at
                .is_none()
        );
    }

    #[tokio::test]
    async fn connection_configuration_cannot_change_during_apply() {
        let (_directory, state) = test_state().await;
        Repository::new(&state)
            .save_connections(&[saved_connection("before-sync")])
            .await
            .unwrap();
        let _run_guard = state
            .panel_sync
            .connection_lock("11111111-1111-4111-8111-111111111111")
            .await
            .lock_owned()
            .await;

        let result = update(
            &state,
            "11111111-1111-4111-8111-111111111111",
            ConnectionUpdateInput {
                name: "Changed".to_string(),
                base_url: "https://other.example.test".to_string(),
                api_path: Some("/api".to_string()),
                allow_invalid_tls: false,
                grouping: GroupingConfig::default(),
                auto_sync: AutoSyncConfig::default(),
                credential: None,
                clear_credential: false,
            },
        )
        .await;
        assert!(matches!(result, Err(ServiceError::Conflict(_))));
        assert_eq!(
            Repository::new(&state).connections().await.unwrap()[0].base_url,
            "https://nav.example.test"
        );
    }

    #[tokio::test]
    async fn configuration_changes_pause_but_do_not_disable_automatic_sync() {
        let (_directory, state) = test_state().await;
        let mut connection = saved_connection("before-change");
        connection.verified_at = Some("2026-08-19T00:00:00Z".to_string());
        Repository::new(&state)
            .save_connections(&[connection])
            .await
            .unwrap();
        credential_store(&state)
            .write("11111111-1111-4111-8111-111111111111", "saved-secret")
            .unwrap();

        let updated = update(
            &state,
            "11111111-1111-4111-8111-111111111111",
            ConnectionUpdateInput {
                name: "NAS".to_string(),
                base_url: "https://changed.example.test".to_string(),
                api_path: Some("/index.php?c=api".to_string()),
                allow_invalid_tls: false,
                grouping: GroupingConfig::default(),
                auto_sync: AutoSyncConfig::default(),
                credential: None,
                clear_credential: false,
            },
        )
        .await
        .unwrap();

        assert!(updated.auto_sync.enabled);
        assert!(updated.verified_at.is_none());
        let decorated = Repository::new(&state).decorate(updated).await.unwrap();
        assert!(decorated.next_sync_at.is_none());
    }

    #[test]
    fn validation_rejects_ambiguous_base_urls_and_oversized_group_names() {
        assert!(normalized_base_url("https://panel.example.test?token=secret").is_err());
        let grouping = GroupingConfig {
            single_group_name: "x".repeat(81),
            ..GroupingConfig::default()
        };
        assert!(
            validate_common(
                "NAS",
                "https://panel.example.test",
                Some("/api"),
                &grouping,
                &AutoSyncConfig::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn credential_clear_and_replacement_are_mutually_exclusive() {
        assert!(validate_credential_update(true, Some("replacement-token")).is_err());
        assert!(validate_credential_update(true, Some("   ")).is_ok());
        assert!(validate_credential_update(false, Some("replacement-token")).is_ok());
    }

    #[test]
    fn root_api_path_is_preserved() {
        assert_eq!(
            normalize_api_path(PanelProvider::VanNav, Some("/")).unwrap(),
            "/"
        );
    }

    #[test]
    fn automatic_sync_defaults_to_enabled() {
        assert!(AutoSyncConfig::default().enabled);
        let parsed: AutoSyncConfig = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.interval_minutes, 60);
    }
}
