use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{state::AppState, time_utils::now_iso};

use super::{
    domain::*,
    local::{self, LOCAL_TARGET_ID},
    repository::{LocalSettingsRecord, LocalSettingsRepository, TargetRepository},
    runtime::{MAX_TARGETS, SessionStartup, SessionStartupBackend},
    secrets::{CredentialBundle, CredentialKind, TerminalSecretStore},
    ssh::{RusshConnector, SshConnector, SshCredential},
};

const MAX_PASSWORD_BYTES: usize = 4 * 1024;
const MAX_PRIVATE_KEY_BYTES: usize = 256 * 1024;
const MAX_PASSPHRASE_BYTES: usize = 4 * 1024;

pub async fn targets(state: &AppState) -> TerminalResult<Vec<TerminalTarget>> {
    let secrets = TerminalSecretStore::from_state(state);
    TargetRepository::new(state)
        .list()
        .await?
        .into_iter()
        .map(|target| decorate_target(target, &secrets))
        .collect()
}

pub async fn target(state: &AppState, id: &str) -> TerminalResult<TerminalTarget> {
    let target = TargetRepository::new(state)
        .get(id)
        .await?
        .ok_or_else(target_not_found)?;
    decorate_target(target, &TerminalSecretStore::from_state(state))
}

pub async fn create_target(
    state: &AppState,
    input: TargetCreateInput,
) -> TerminalResult<TerminalTarget> {
    validate_target_fields(&input.name, &input.host, input.port, &input.username)?;
    validate_trusted_key(input.trusted_host_key.as_ref())?;
    validate_secret_mutations(input.auth_method, &input.credential, &input.passphrase)?;
    let _guard = state.terminal.catalog_operation().await;
    let repository = TargetRepository::new(state);
    if repository.list().await?.len() >= MAX_TARGETS {
        return Err(TerminalError::new(
            TerminalErrorCode::Conflict,
            "terminal target limit reached",
        ));
    }
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let record = TargetRecord {
        id: id.clone(),
        name: input.name.trim().to_string(),
        host: input.host.trim().to_string(),
        port: input.port,
        username: input.username.trim().to_string(),
        auth_method: input.auth_method,
        trusted_host_key: input.trusted_host_key,
        revision: 1,
        last_verified_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    let secrets = TerminalSecretStore::from_state(state);
    let last_verified_at = consume_verification_token(
        state,
        input.verification_token.as_deref(),
        None,
        &record,
        || {
            credential_for_request(
                &secrets,
                &record,
                None,
                &input.credential,
                &input.passphrase,
            )
        },
    )
    .await?;
    let mut record = record;
    record.last_verified_at = last_verified_at;
    if let Err(error) = apply_credential(
        &secrets,
        &record.id,
        record.auth_method,
        record.revision,
        None,
        &input.credential,
        &input.passphrase,
    ) {
        let _ = secrets.delete_target(&record.id);
        return Err(error);
    }
    if let Err(error) = repository.insert(record.clone()).await {
        let _ = secrets.delete_target(&record.id);
        return Err(error);
    }
    audit_event(
        state,
        "target_created",
        Some(&record.id),
        None,
        Some(record.revision),
        None,
    )
    .await;
    if record.trusted_host_key.is_some() {
        audit_event(
            state,
            "host_key_confirmed",
            Some(&record.id),
            None,
            Some(record.revision),
            None,
        )
        .await;
    }
    tracing::info!(target_id = %record.id, "terminal target created");
    decorate_target(record, &secrets)
}

pub async fn update_target(
    state: &AppState,
    id: &str,
    input: TargetUpdateInput,
    force: bool,
    confirmation_token: Option<&str>,
) -> TerminalResult<TerminalTarget> {
    validate_target_fields(&input.name, &input.host, input.port, &input.username)?;
    validate_trusted_key(input.trusted_host_key.as_ref())?;
    validate_secret_mutations(input.auth_method, &input.credential, &input.passphrase)?;
    let _guard = state.terminal.target_operation(id).await;
    let repository = TargetRepository::new(state);
    let existing = repository.get(id).await?.ok_or_else(target_not_found)?;
    if existing.revision != input.revision {
        return Err(TerminalError::new(
            TerminalErrorCode::TargetRevisionConflict,
            "terminal target was modified by another request",
        ));
    }
    if existing.auth_method != input.auth_method && input.credential.action == SecretAction::Keep {
        return Err(TerminalError::invalid(
            "credential must be replaced or cleared when authentication method changes",
        ));
    }
    let connection_changed = existing.host != input.host.trim()
        || existing.port != input.port
        || existing.username != input.username.trim()
        || existing.auth_method != input.auth_method
        || existing.trusted_host_key != input.trusted_host_key
        || input.credential.action != SecretAction::Keep
        || input.passphrase.action != SecretAction::Keep;
    let host_key_confirmed =
        existing.trusted_host_key != input.trusted_host_key && input.trusted_host_key.is_some();
    let now = now_iso();
    let mut record = TargetRecord {
        id: id.to_string(),
        name: input.name.trim().to_string(),
        host: input.host.trim().to_string(),
        port: input.port,
        username: input.username.trim().to_string(),
        auth_method: input.auth_method,
        trusted_host_key: input.trusted_host_key.clone(),
        revision: existing.revision.saturating_add(1),
        last_verified_at: None,
        created_at: existing.created_at.clone(),
        updated_at: now,
    };
    let active_session_ids = if connection_changed {
        confirm_active_session_mutation(state, id, existing.revision, force, confirmation_token)
            .await?
    } else {
        Vec::new()
    };
    let secrets = TerminalSecretStore::from_state(state);
    let verified_at = consume_verification_token(
        state,
        input.verification_token.as_deref(),
        Some(id),
        &record,
        || {
            credential_for_request(
                &secrets,
                &record,
                Some(&existing),
                &input.credential,
                &input.passphrase,
            )
        },
    )
    .await?;
    record.last_verified_at = verified_at.or_else(|| {
        (!connection_changed)
            .then(|| existing.last_verified_at.clone())
            .flatten()
    });
    if connection_changed && !active_session_ids.is_empty() {
        state.terminal.terminate_target(id).await;
    }
    let snapshot = SecretSnapshot::capture(&secrets, id)?;
    if let Err(error) = apply_credential(
        &secrets,
        id,
        input.auth_method,
        record.revision,
        Some(&existing),
        &input.credential,
        &input.passphrase,
    ) {
        snapshot.restore(&secrets, id);
        return Err(error);
    }
    let _catalog_guard = state.terminal.catalog_operation().await;
    if let Err(error) = repository.replace(record.clone()).await {
        snapshot.restore(&secrets, id);
        return Err(error);
    }
    audit_event(
        state,
        "target_updated",
        Some(id),
        None,
        Some(record.revision),
        None,
    )
    .await;
    if host_key_confirmed {
        audit_event(
            state,
            "host_key_confirmed",
            Some(id),
            None,
            Some(record.revision),
            None,
        )
        .await;
    }
    tracing::info!(
        target_id = id,
        revision = record.revision,
        connection_changed,
        "terminal target updated"
    );
    decorate_target(record, &secrets)
}

pub async fn delete_target(
    state: &AppState,
    id: &str,
    expected_revision: u64,
    force: bool,
    confirmation_token: Option<&str>,
) -> TerminalResult<()> {
    let _guard = state.terminal.target_operation(id).await;
    let repository = TargetRepository::new(state);
    let existing = repository.get(id).await?.ok_or_else(target_not_found)?;
    if existing.revision != expected_revision {
        return Err(TerminalError::new(
            TerminalErrorCode::TargetRevisionConflict,
            "terminal target was modified by another request",
        ));
    }
    confirm_active_session_mutation(state, id, existing.revision, force, confirmation_token)
        .await?;
    state.terminal.terminate_target(id).await;
    let secrets = TerminalSecretStore::from_state(state);
    let snapshot = SecretSnapshot::capture(&secrets, id)?;
    if let Err(error) = secrets.delete_target(id) {
        snapshot.restore(&secrets, id);
        return Err(error);
    }
    let _catalog_guard = state.terminal.catalog_operation().await;
    if let Err(error) = repository.delete(id).await {
        snapshot.restore(&secrets, id);
        return Err(error);
    }
    audit_event(state, "target_deleted", Some(id), None, None, None).await;
    tracing::info!(target_id = id, "terminal target deleted");
    Ok(())
}

async fn confirm_active_session_mutation(
    state: &AppState,
    target_id: &str,
    target_revision: u64,
    force: bool,
    confirmation_token: Option<&str>,
) -> TerminalResult<Vec<String>> {
    let active_session_ids = state.terminal.active_session_ids(target_id).await;
    if active_session_ids.is_empty() {
        return Ok(active_session_ids);
    }
    let confirmed = if force {
        match confirmation_token {
            Some(token) if token.len() <= 128 => {
                state
                    .terminal
                    .consume_force_confirmation(
                        token,
                        target_id,
                        target_revision,
                        &active_session_ids,
                    )
                    .await
            }
            _ => false,
        }
    } else {
        false
    };
    if confirmed {
        return Ok(active_session_ids);
    }
    let count = active_session_ids.len();
    let token = state
        .terminal
        .issue_force_confirmation(target_id, target_revision, active_session_ids)
        .await;
    Err(TerminalError::new(
        TerminalErrorCode::Conflict,
        format!("terminal target has {count} active session(s)"),
    )
    .with_active_session_count(count)
    .with_confirmation_token(token))
}

pub async fn probe_host_key(input: ProbeHostKeyInput) -> TerminalResult<HostKeyProbeResult> {
    RusshConnector.probe_host_key(&input.host, input.port).await
}

pub async fn test_connection(
    state: &AppState,
    input: TerminalTestConnectionInput,
) -> TerminalResult<ConnectionTestResult> {
    // A saved target's metadata and encrypted credential bundle form one
    // logical configuration. Keep the target lock from the first read through
    // authentication so an update can never mix a new secret with an old
    // host/fingerprint (or the inverse).
    let _target_guard = match input.target_id.as_deref() {
        Some(target_id) => Some(state.terminal.target_operation(target_id).await),
        None => None,
    };
    let test_auth_method = input.draft.as_ref().map(|draft| draft.auth_method);
    let verification_scope = input.target_id.clone();
    let testing_saved_configuration = input.draft.is_none()
        && input.credential.action == SecretAction::Keep
        && input.passphrase.action == SecretAction::Keep;
    let saved = if let Some(id) = input.target_id.as_deref() {
        Some(
            TargetRepository::new(state)
                .get(id)
                .await?
                .ok_or_else(target_not_found)?,
        )
    } else {
        None
    };
    let record = match input.draft {
        Some(draft) => {
            validate_target_fields("draft", &draft.host, draft.port, &draft.username)?;
            validate_trusted_key(draft.trusted_host_key.as_ref())?;
            TargetRecord {
                id: saved
                    .as_ref()
                    .map(|target| target.id.clone())
                    .unwrap_or_else(|| "draft".to_string()),
                name: "draft".to_string(),
                host: draft.host.trim().to_string(),
                port: draft.port,
                username: draft.username.trim().to_string(),
                auth_method: draft.auth_method,
                trusted_host_key: draft.trusted_host_key,
                revision: saved.as_ref().map_or(0, |target| target.revision),
                last_verified_at: None,
                created_at: String::new(),
                updated_at: String::new(),
            }
        }
        None => saved.clone().ok_or_else(|| {
            TerminalError::invalid("targetId or draft is required for connection test")
        })?,
    };
    validate_secret_mutations(
        test_auth_method.unwrap_or(record.auth_method),
        &input.credential,
        &input.passphrase,
    )?;
    let credential = credential_for_request(
        &TerminalSecretStore::from_state(state),
        &record,
        saved.as_ref(),
        &input.credential,
        &input.passphrase,
    )?;
    let verification_fingerprint =
        connection_verification_fingerprint(verification_scope.as_deref(), &record, &credential);
    let latency_ms = match RusshConnector.test_connection(&record, credential).await {
        Ok(latency_ms) => latency_ms,
        Err(error) => {
            audit_event(
                state,
                "connection_test_failed",
                verification_scope.as_deref(),
                None,
                saved.as_ref().map(|target| target.revision),
                Some(error.code),
            )
            .await;
            return Err(error);
        }
    };
    if testing_saved_configuration {
        let _catalog_guard = state.terminal.catalog_operation().await;
        let repository = TargetRepository::new(state);
        if let Some(mut current) = repository.get(&record.id).await?
            && current.revision == record.revision
            && same_connection_config(&current, &record)
        {
            current.last_verified_at = Some(now_iso());
            current.updated_at = now_iso();
            repository.replace(current).await?;
        }
    }
    tracing::info!(target_id = %record.id, latency_ms, "terminal SSH connection tested");
    audit_event(
        state,
        "connection_test_succeeded",
        verification_scope.as_deref(),
        None,
        saved.as_ref().map(|target| target.revision),
        None,
    )
    .await;
    let verification_token = state
        .terminal
        .issue_verification(verification_fingerprint)
        .await;
    Ok(ConnectionTestResult {
        success: true,
        latency_ms,
        verification_token,
    })
}

async fn consume_verification_token(
    state: &AppState,
    token: Option<&str>,
    scope: Option<&str>,
    target: &TargetRecord,
    credential: impl FnOnce() -> TerminalResult<SshCredential>,
) -> TerminalResult<Option<String>> {
    let Some(token) = token.map(str::trim).filter(|token| !token.is_empty()) else {
        return Ok(None);
    };
    if token.len() > 128 {
        return Err(TerminalError::invalid(
            "connection verification token is invalid",
        ));
    }
    let credential = credential()?;
    let fingerprint = connection_verification_fingerprint(scope, target, &credential);
    if !state
        .terminal
        .consume_verification(token, &fingerprint)
        .await
    {
        return Err(TerminalError::invalid(
            "connection verification token is invalid or expired",
        ));
    }
    Ok(Some(now_iso()))
}

fn connection_verification_fingerprint(
    scope: Option<&str>,
    target: &TargetRecord,
    credential: &SshCredential,
) -> String {
    let mut digest = Sha256::new();
    digest_field(&mut digest, b"fn-knock-terminal-verification-v1");
    digest_field(&mut digest, scope.unwrap_or_default().as_bytes());
    digest_field(&mut digest, target.host.as_bytes());
    digest_field(&mut digest, &target.port.to_be_bytes());
    digest_field(&mut digest, target.username.as_bytes());
    digest_field(
        &mut digest,
        match target.auth_method {
            AuthMethod::Password => b"password",
            AuthMethod::PrivateKey => b"private-key",
        },
    );
    if let Some(key) = target.trusted_host_key.as_ref() {
        digest_field(&mut digest, key.algorithm.as_bytes());
        digest_field(&mut digest, key.fingerprint.as_bytes());
    } else {
        digest_field(&mut digest, b"");
        digest_field(&mut digest, b"");
    }
    match credential {
        SshCredential::Password(password) => {
            digest_field(&mut digest, b"password");
            digest_field(&mut digest, password.as_bytes());
        }
        SshCredential::PrivateKey { key, passphrase } => {
            digest_field(&mut digest, b"private-key");
            digest_field(&mut digest, key.as_bytes());
            digest_field(
                &mut digest,
                passphrase.as_deref().unwrap_or_default().as_bytes(),
            );
        }
    }
    hex::encode(digest.finalize())
}

fn digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn same_connection_config(left: &TargetRecord, right: &TargetRecord) -> bool {
    left.host == right.host
        && left.port == right.port
        && left.username == right.username
        && left.auth_method == right.auth_method
        && left.trusted_host_key == right.trusted_host_key
}

pub async fn sessions(state: &AppState) -> SessionListResult {
    state.terminal.list().await
}

pub async fn local_terminal_status(state: &AppState) -> TerminalResult<LocalTerminalStatus> {
    let settings = LocalSettingsRepository::new(state).get().await?;
    Ok(local::status(state, settings))
}

pub async fn update_local_terminal(
    state: &AppState,
    input: LocalTerminalSettingsInput,
    force: bool,
    confirmation_token: Option<&str>,
) -> TerminalResult<LocalTerminalStatus> {
    let _target_guard = state.terminal.target_operation(LOCAL_TARGET_ID).await;
    let repository = LocalSettingsRepository::new(state);
    let current = repository.get().await?;
    if current.revision != input.revision {
        return Err(TerminalError::new(
            TerminalErrorCode::LocalTerminalRevisionConflict,
            "local terminal settings were modified by another request",
        ));
    }
    if current.enabled == input.enabled {
        return Ok(local::status(state, current));
    }
    if input.enabled {
        let current_status = local::status(state, current);
        if !current_status.supported {
            return Err(TerminalError::new(
                TerminalErrorCode::LocalTerminalUnsupported,
                "local terminal is not supported on this platform",
            ));
        }
        if !current_status.ready {
            return Err(TerminalError::new(
                TerminalErrorCode::LocalShellUnavailable,
                "no supported local login shell is available",
            ));
        }
        if !input.acknowledge_risk {
            return Err(TerminalError::new(
                TerminalErrorCode::LocalTerminalRiskAcknowledgementRequired,
                "local terminal execution risk must be acknowledged",
            ));
        }
    } else {
        confirm_active_session_mutation(
            state,
            LOCAL_TARGET_ID,
            current.revision,
            force,
            confirmation_token,
        )
        .await?;
    }
    let next = LocalSettingsRecord {
        enabled: input.enabled,
        revision: current.revision.saturating_add(1).max(1),
    };
    repository.save(next).await?;
    if !next.enabled {
        state.terminal.terminate_target(LOCAL_TARGET_ID).await;
    }
    let action = if next.enabled {
        "local_terminal_enabled"
    } else {
        "local_terminal_disabled"
    };
    audit_event(
        state,
        action,
        Some(LOCAL_TARGET_ID),
        None,
        Some(next.revision),
        None,
    )
    .await;
    tracing::info!(
        enabled = next.enabled,
        revision = next.revision,
        "local terminal setting updated"
    );
    Ok(local::status(state, next))
}

pub async fn create_session(
    state: &AppState,
    target_id: &str,
    input: CreateSessionInput,
) -> TerminalResult<TerminalSession> {
    let target_guard = state.terminal.target_operation(target_id).await;
    let target = TargetRepository::new(state)
        .get(target_id)
        .await?
        .ok_or_else(target_not_found)?;
    let credential = stored_credential(&TerminalSecretStore::from_state(state), &target)?;
    let target_revision = target.revision;
    let cols = input.cols.unwrap_or(120).clamp(40, 400);
    let rows = input.rows.unwrap_or(32).clamp(12, 200);
    let title = match input.title {
        Some(title) => sanitize_session_title(&title)?,
        None => default_session_title(state, &target).await,
    };
    let pending = state
        .terminal
        .begin_session(target.id.clone(), title, cols, rows)
        .await?;
    let session = state
        .terminal
        .start_session(SessionStartup {
            pending,
            backend: SessionStartupBackend::Ssh { target, credential },
            initial_cols: cols,
            initial_rows: rows,
            shutdown: state.shutdown.child_token(),
            target_guard,
            audit_state: Some(state.clone()),
        })
        .await?;
    tracing::info!(target_id, session_id = %session.id, "terminal session creation started");
    audit_event(
        state,
        "session_creation_started",
        Some(target_id),
        Some(&session.id),
        Some(target_revision),
        None,
    )
    .await;
    Ok(session)
}

pub async fn create_local_session(
    state: &AppState,
    input: CreateSessionInput,
) -> TerminalResult<TerminalSession> {
    let mut settings_revision = None;
    let result = create_local_session_inner(state, input, &mut settings_revision).await;
    if let Err(error) = result.as_ref() {
        tracing::warn!(
            target_id = LOCAL_TARGET_ID,
            error_code = %error.code,
            "local terminal session creation rejected"
        );
        audit_event(
            state,
            "session_creation_failed",
            Some(LOCAL_TARGET_ID),
            None,
            settings_revision,
            Some(error.code),
        )
        .await;
    }
    result
}

async fn create_local_session_inner(
    state: &AppState,
    input: CreateSessionInput,
    settings_revision: &mut Option<u64>,
) -> TerminalResult<TerminalSession> {
    let target_guard = state.terminal.target_operation(LOCAL_TARGET_ID).await;
    let descriptor = local::descriptor(state)?;
    let settings = LocalSettingsRepository::new(state).get().await?;
    *settings_revision = Some(settings.revision);
    if !settings.enabled {
        return Err(TerminalError::new(
            TerminalErrorCode::LocalTerminalDisabled,
            "local terminal is disabled",
        ));
    }
    let cols = input.cols.unwrap_or(120).clamp(40, 400);
    let rows = input.rows.unwrap_or(32).clamp(12, 200);
    let title = match input.title {
        Some(title) => sanitize_session_title(&title)?,
        None => default_local_session_title(state).await,
    };
    let pending = state
        .terminal
        .begin_session(LOCAL_TARGET_ID.to_string(), title, cols, rows)
        .await?;
    let execution_identity = descriptor.execution_identity.clone();
    let privileged = descriptor.privileged;
    let session = state
        .terminal
        .start_session(SessionStartup {
            pending,
            backend: SessionStartupBackend::Local { descriptor },
            initial_cols: cols,
            initial_rows: rows,
            shutdown: state.shutdown.child_token(),
            target_guard,
            audit_state: Some(state.clone()),
        })
        .await?;
    tracing::info!(
        target_id = LOCAL_TARGET_ID,
        session_id = %session.id,
        %execution_identity,
        privileged,
        "local terminal session creation started"
    );
    audit_event(
        state,
        "session_creation_started",
        Some(LOCAL_TARGET_ID),
        Some(&session.id),
        Some(settings.revision),
        None,
    )
    .await;
    Ok(session)
}

pub async fn terminate_session(state: &AppState, id: &str) -> TerminalResult<()> {
    state.terminal.terminate(id).await
}

pub async fn rename_session(
    state: &AppState,
    id: &str,
    input: RenameSessionInput,
) -> TerminalResult<TerminalSession> {
    let session = state.terminal.rename(id, &input.title).await?;
    tracing::info!(session_id = id, "terminal session renamed");
    Ok(session)
}

async fn default_session_title(state: &AppState, target: &TargetRecord) -> String {
    let existing = state
        .terminal
        .list()
        .await
        .sessions
        .into_iter()
        .filter(|session| session.target_id == target.id)
        .count();
    format!("{} {}", target.name, existing.saturating_add(1))
}

async fn default_local_session_title(state: &AppState) -> String {
    let existing = state
        .terminal
        .list()
        .await
        .sessions
        .into_iter()
        .filter(|session| session.target_id == LOCAL_TARGET_ID)
        .count();
    format!("Local {}", existing.saturating_add(1))
}

fn decorate_target(
    record: TargetRecord,
    secrets: &TerminalSecretStore,
) -> TerminalResult<TerminalTarget> {
    let bundle = secrets.read_bundle(&record.id)?;
    let credential_kind = match record.auth_method {
        AuthMethod::Password => CredentialKind::Password,
        AuthMethod::PrivateKey => CredentialKind::PrivateKey,
    };
    let bundle_matches_target =
        bundle.auth_method == Some(record.auth_method) && bundle.target_revision == record.revision;
    Ok(TerminalTarget {
        id: record.id.clone(),
        name: record.name,
        host: record.host,
        port: record.port,
        username: record.username,
        auth_method: record.auth_method,
        trusted_host_key: record.trusted_host_key,
        credential_configured: bundle_matches_target
            && bundle_value(&bundle, credential_kind).is_some(),
        passphrase_configured: bundle_matches_target
            && record.auth_method == AuthMethod::PrivateKey
            && bundle.passphrase.is_some(),
        revision: record.revision,
        last_verified_at: record.last_verified_at,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

fn bundle_value(bundle: &CredentialBundle, kind: CredentialKind) -> Option<&[u8]> {
    match kind {
        CredentialKind::Password => bundle.password.as_deref(),
        CredentialKind::PrivateKey => bundle.private_key.as_deref(),
        #[cfg(test)]
        CredentialKind::Passphrase => bundle.passphrase.as_deref(),
    }
}

fn validate_target_fields(name: &str, host: &str, port: u16, username: &str) -> TerminalResult<()> {
    validate_short_text(name, 80, "target name")?;
    validate_short_text(host, 253, "SSH host")?;
    validate_short_text(username, 128, "SSH username")?;
    if port == 0 {
        return Err(TerminalError::invalid("SSH port is invalid"));
    }
    Ok(())
}

fn validate_short_text(value: &str, max: usize, label: &str) -> TerminalResult<()> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(TerminalError::invalid(format!("{label} is invalid")));
    }
    Ok(())
}

fn validate_trusted_key(key: Option<&TrustedHostKey>) -> TerminalResult<()> {
    let Some(key) = key else {
        return Ok(());
    };
    if key.algorithm.trim().is_empty()
        || key.algorithm.len() > 64
        || key.fingerprint.len() > 128
        || !key.fingerprint.starts_with("SHA256:")
    {
        return Err(TerminalError::invalid("trusted SSH host key is invalid"));
    }
    Ok(())
}

fn validate_secret_mutations(
    auth_method: AuthMethod,
    credential: &CredentialMutation,
    passphrase: &PassphraseMutation,
) -> TerminalResult<()> {
    let credential_limit = match auth_method {
        AuthMethod::Password => MAX_PASSWORD_BYTES,
        AuthMethod::PrivateKey => MAX_PRIVATE_KEY_BYTES,
    };
    validate_secret_mutation(
        "credential",
        credential.action,
        credential.secret.as_deref(),
        credential_limit,
    )?;
    validate_secret_mutation(
        "private key passphrase",
        passphrase.action,
        passphrase.secret.as_deref(),
        MAX_PASSPHRASE_BYTES,
    )?;
    if auth_method == AuthMethod::Password && passphrase.action == SecretAction::Replace {
        return Err(TerminalError::invalid(
            "passphrase cannot be configured for password authentication",
        ));
    }
    if auth_method == AuthMethod::PrivateKey
        && credential.action == SecretAction::Clear
        && passphrase.action == SecretAction::Replace
    {
        return Err(TerminalError::invalid(
            "passphrase cannot be replaced when the private key is cleared",
        ));
    }
    Ok(())
}

fn validate_secret_mutation(
    label: &str,
    action: SecretAction,
    secret: Option<&str>,
    max_bytes: usize,
) -> TerminalResult<()> {
    match action {
        SecretAction::Replace => {
            let secret = secret
                .filter(|value| !value.is_empty())
                .ok_or_else(|| TerminalError::invalid(format!("{label} secret is required")))?;
            if secret.len() > max_bytes {
                return Err(TerminalError::invalid(format!(
                    "{label} secret exceeds the size limit"
                )));
            }
        }
        SecretAction::Keep | SecretAction::Clear if secret.is_some() => {
            return Err(TerminalError::invalid(format!(
                "{label} secret is only accepted with replace"
            )));
        }
        SecretAction::Keep | SecretAction::Clear => {}
    }
    Ok(())
}

fn sanitize_session_title(value: &str) -> TerminalResult<String> {
    validate_short_text(value, 80, "session title")?;
    Ok(value.trim().to_string())
}

fn apply_credential(
    store: &TerminalSecretStore,
    target_id: &str,
    auth_method: AuthMethod,
    target_revision: u64,
    previous_target: Option<&TargetRecord>,
    credential: &CredentialMutation,
    passphrase: &PassphraseMutation,
) -> TerminalResult<()> {
    validate_secret_mutations(auth_method, credential, passphrase)?;
    store.update_bundle(target_id, |bundle| {
        let retains_existing_secret = credential.action == SecretAction::Keep
            || (auth_method == AuthMethod::PrivateKey
                && credential.action != SecretAction::Clear
                && passphrase.action == SecretAction::Keep);
        let has_existing_secret = bundle.password.is_some()
            || bundle.private_key.is_some()
            || bundle.passphrase.is_some();
        if retains_existing_secret
            && has_existing_secret
            && previous_target.is_none_or(|previous| {
                bundle.auth_method != Some(previous.auth_method)
                    || bundle.target_revision != previous.revision
            })
        {
            return Err(credential_missing());
        }
        match auth_method {
            AuthMethod::Password => {
                match credential.action {
                    SecretAction::Keep => {}
                    SecretAction::Clear => bundle.password = None,
                    SecretAction::Replace => {
                        let secret = credential
                            .secret
                            .as_deref()
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| {
                                TerminalError::invalid("credential secret is required")
                            })?;
                        bundle.password = Some(secret.as_bytes().to_vec());
                    }
                }
                bundle.private_key = None;
                bundle.passphrase = None;
            }
            AuthMethod::PrivateKey => {
                let credential_cleared = credential.action == SecretAction::Clear;
                match credential.action {
                    SecretAction::Keep => {}
                    SecretAction::Clear => bundle.private_key = None,
                    SecretAction::Replace => {
                        let secret = credential
                            .secret
                            .as_deref()
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| {
                                TerminalError::invalid("credential secret is required")
                            })?;
                        bundle.private_key = Some(secret.as_bytes().to_vec());
                    }
                }
                bundle.password = None;
                if credential_cleared {
                    bundle.passphrase = None;
                } else {
                    match passphrase.action {
                        SecretAction::Keep => {}
                        SecretAction::Clear => bundle.passphrase = None,
                        SecretAction::Replace => {
                            let secret = passphrase
                                .secret
                                .as_deref()
                                .filter(|value| !value.is_empty())
                                .ok_or_else(|| {
                                    TerminalError::invalid("private key passphrase is required")
                                })?;
                            bundle.passphrase = Some(secret.as_bytes().to_vec());
                        }
                    }
                }
            }
        }
        bundle.auth_method = Some(auth_method);
        bundle.target_revision = target_revision;
        Ok(())
    })
}

fn credential_for_request(
    store: &TerminalSecretStore,
    target: &TargetRecord,
    stored_target: Option<&TargetRecord>,
    credential: &CredentialMutation,
    passphrase: &PassphraseMutation,
) -> TerminalResult<SshCredential> {
    validate_secret_mutations(target.auth_method, credential, passphrase)?;
    let needs_bundle = credential.action == SecretAction::Keep
        || (target.auth_method == AuthMethod::PrivateKey
            && passphrase.action == SecretAction::Keep
            && stored_target.is_some_and(|stored| stored.auth_method == AuthMethod::PrivateKey));
    let stored_bundle = needs_bundle
        .then(|| store.read_bundle(&target.id))
        .transpose()?;
    if let (Some(stored), Some(bundle)) = (stored_target, stored_bundle.as_ref())
        && (bundle.auth_method != Some(stored.auth_method)
            || bundle.target_revision != stored.revision)
    {
        return Err(credential_missing());
    }
    let secret = match credential.action {
        SecretAction::Keep => {
            let kind = match target.auth_method {
                AuthMethod::Password => CredentialKind::Password,
                AuthMethod::PrivateKey => CredentialKind::PrivateKey,
            };
            stored_bundle
                .as_ref()
                .and_then(|bundle| bundle_value(bundle, kind))
                .map(<[u8]>::to_vec)
                .ok_or_else(credential_missing)?
        }
        SecretAction::Clear => return Err(credential_missing()),
        SecretAction::Replace => credential
            .secret
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| TerminalError::invalid("credential secret is required"))?
            .as_bytes()
            .to_vec(),
    };
    match target.auth_method {
        AuthMethod::Password => Ok(SshCredential::Password(
            String::from_utf8(secret)
                .map_err(|_| TerminalError::internal("SSH password is invalid"))?,
        )),
        AuthMethod::PrivateKey => {
            let passphrase = match passphrase.action {
                SecretAction::Keep => stored_bundle
                    .as_ref()
                    .and_then(|bundle| bundle.passphrase.clone())
                    .map(String::from_utf8)
                    .transpose()
                    .map_err(|_| TerminalError::internal("stored SSH passphrase is invalid"))?,
                SecretAction::Clear => None,
                SecretAction::Replace => Some(
                    passphrase
                        .secret
                        .as_ref()
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            TerminalError::invalid("private key passphrase is required")
                        })?
                        .clone(),
                ),
            };
            Ok(SshCredential::PrivateKey {
                key: String::from_utf8(secret)
                    .map_err(|_| TerminalError::internal("SSH private key is invalid"))?,
                passphrase,
            })
        }
    }
}

fn stored_credential(
    store: &TerminalSecretStore,
    target: &TargetRecord,
) -> TerminalResult<SshCredential> {
    let bundle = store.read_bundle(&target.id)?;
    if bundle.auth_method != Some(target.auth_method) || bundle.target_revision != target.revision {
        return Err(credential_missing());
    }
    match target.auth_method {
        AuthMethod::Password => {
            let value = bundle.password.ok_or_else(credential_missing)?;
            let password = String::from_utf8(value)
                .map_err(|_| TerminalError::internal("stored SSH password is invalid"))?;
            Ok(SshCredential::Password(password))
        }
        AuthMethod::PrivateKey => {
            let value = bundle.private_key.ok_or_else(credential_missing)?;
            let key = String::from_utf8(value)
                .map_err(|_| TerminalError::internal("stored SSH private key is invalid"))?;
            let passphrase = bundle
                .passphrase
                .map(String::from_utf8)
                .transpose()
                .map_err(|_| TerminalError::internal("stored SSH passphrase is invalid"))?;
            Ok(SshCredential::PrivateKey { key, passphrase })
        }
    }
}

fn credential_missing() -> TerminalError {
    TerminalError::new(
        TerminalErrorCode::AuthenticationFailed,
        "SSH credential is not configured",
    )
}

fn target_not_found() -> TerminalError {
    TerminalError::new(
        TerminalErrorCode::TargetNotFound,
        "terminal target not found",
    )
}

async fn audit_event(
    state: &AppState,
    action: &str,
    target_id: Option<&str>,
    session_id: Option<&str>,
    revision: Option<u64>,
    error_code: Option<TerminalErrorCode>,
) {
    let error_code = error_code.map(|code| code.to_string());
    let result = if target_id == Some(LOCAL_TARGET_ID) {
        let (execution_identity, privileged) = local::audit_context(state);
        crate::system_events::publish_local_terminal_audit_event(
            state,
            action,
            target_id,
            session_id,
            revision,
            error_code.as_deref(),
            (&execution_identity, privileged),
        )
        .await
    } else {
        crate::system_events::publish_terminal_audit_event(
            state,
            action,
            target_id,
            session_id,
            revision,
            error_code.as_deref(),
        )
        .await
    };
    if let Err(error) = result {
        tracing::warn!(action, %error, "failed to publish terminal audit event");
    }
}

struct SecretSnapshot {
    bundle: CredentialBundle,
}

impl SecretSnapshot {
    fn capture(store: &TerminalSecretStore, target_id: &str) -> TerminalResult<Self> {
        Ok(Self {
            bundle: store.read_bundle(target_id)?,
        })
    }

    fn restore(&self, store: &TerminalSecretStore, target_id: &str) {
        if let Err(error) = store.write_bundle(target_id, &self.bundle) {
            tracing::error!(target_id, %error, "failed to roll back terminal credential mutation");
        }
    }
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
        settings.data_dir = directory.path().join("data");
        settings.runtime_target = "linux".to_string();
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
        settings.internal_rpc_token = "terminal-service-test-token".to_string();
        settings.request_timeout = std::time::Duration::from_millis(100);
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

    fn mutation(action: SecretAction, secret: Option<&str>) -> CredentialMutation {
        CredentialMutation {
            action,
            secret: secret.map(str::to_string),
        }
    }

    fn passphrase(action: SecretAction, secret: Option<&str>) -> PassphraseMutation {
        PassphraseMutation {
            action,
            secret: secret.map(str::to_string),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_terminal_defaults_off_and_requires_risk_acknowledgement() {
        let (_directory, state) = test_state().await;
        let initial = local_terminal_status(&state).await.unwrap();
        assert!(initial.supported);
        assert!(initial.ready);
        assert!(!initial.enabled);
        assert_eq!(initial.revision, 0);
        assert!(initial.shell.is_some());

        let error = update_local_terminal(
            &state,
            LocalTerminalSettingsInput {
                enabled: true,
                revision: 0,
                acknowledge_risk: false,
            },
            false,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.code,
            TerminalErrorCode::LocalTerminalRiskAcknowledgementRequired
        );

        let enabled = update_local_terminal(
            &state,
            LocalTerminalSettingsInput {
                enabled: true,
                revision: 0,
                acknowledge_risk: true,
            },
            false,
            None,
        )
        .await
        .unwrap();
        assert!(enabled.enabled);
        assert_eq!(enabled.revision, 1);
        assert_eq!(local_terminal_status(&state).await.unwrap(), enabled);

        let stale = update_local_terminal(
            &state,
            LocalTerminalSettingsInput {
                enabled: false,
                revision: 0,
                acknowledge_risk: false,
            },
            false,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(stale.code, TerminalErrorCode::LocalTerminalRevisionConflict);
        state.terminal.shutdown_all().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejected_local_session_creation_is_audited() {
        let (_directory, state) = test_state().await;
        let error = create_local_session(
            &state,
            CreateSessionInput {
                title: None,
                cols: None,
                rows: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, TerminalErrorCode::LocalTerminalDisabled);

        let page = state
            .storage
            .store
            .list_system_events(1, 20, "", Some("FN_EVENT_TERMINAL_AUDIT"), None, None)
            .await
            .unwrap();
        let events = page
            .get("events")
            .and_then(serde_json::Value::as_array)
            .expect("terminal audit events");
        assert!(events.iter().any(|event| {
            event.get("payload").is_some_and(|payload| {
                payload.get("action").and_then(serde_json::Value::as_str)
                    == Some("session_creation_failed")
                    && payload.get("backend").and_then(serde_json::Value::as_str) == Some("local")
                    && payload
                        .get("error_code")
                        .and_then(serde_json::Value::as_str)
                        == Some("local_terminal_disabled")
            })
        }));
        state.terminal.shutdown_all().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn disabling_local_terminal_reuses_active_session_confirmation() {
        let (_directory, state) = test_state().await;
        let enabled = update_local_terminal(
            &state,
            LocalTerminalSettingsInput {
                enabled: true,
                revision: 0,
                acknowledge_risk: true,
            },
            false,
            None,
        )
        .await
        .unwrap();
        state
            .terminal
            .reserve_active_test_session(LOCAL_TARGET_ID)
            .await
            .unwrap();

        let conflict = update_local_terminal(
            &state,
            LocalTerminalSettingsInput {
                enabled: false,
                revision: enabled.revision,
                acknowledge_risk: false,
            },
            false,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(conflict.code, TerminalErrorCode::Conflict);
        assert_eq!(conflict.active_session_count, Some(1));

        let disabled = update_local_terminal(
            &state,
            LocalTerminalSettingsInput {
                enabled: false,
                revision: enabled.revision,
                acknowledge_risk: false,
            },
            true,
            conflict.confirmation_token.as_deref(),
        )
        .await
        .unwrap();
        assert!(!disabled.enabled);
        assert_eq!(state.terminal.active_counts(Some(LOCAL_TARGET_ID)).await, 0);
        state.terminal.shutdown_all().await;
    }

    #[test]
    fn clearing_private_key_also_clears_orphan_passphrase() {
        let directory = tempfile::tempdir().unwrap();
        let store = TerminalSecretStore::new(directory.path());
        store
            .write("target-a", CredentialKind::PrivateKey, b"key")
            .unwrap();
        store
            .write("target-a", CredentialKind::Passphrase, b"phrase")
            .unwrap();
        apply_credential(
            &store,
            "target-a",
            AuthMethod::PrivateKey,
            1,
            None,
            &mutation(SecretAction::Clear, None),
            &passphrase(SecretAction::Keep, None),
        )
        .unwrap();
        assert!(
            store
                .read("target-a", CredentialKind::PrivateKey)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .read("target-a", CredentialKind::Passphrase)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn password_auth_rejects_passphrase_replace_and_clears_old_value() {
        let directory = tempfile::tempdir().unwrap();
        let store = TerminalSecretStore::new(directory.path());
        assert!(
            apply_credential(
                &store,
                "target-a",
                AuthMethod::Password,
                1,
                None,
                &mutation(SecretAction::Replace, Some("password")),
                &passphrase(SecretAction::Replace, Some("invalid")),
            )
            .is_err()
        );
        store
            .write("target-a", CredentialKind::Passphrase, b"old")
            .unwrap();
        apply_credential(
            &store,
            "target-a",
            AuthMethod::Password,
            1,
            None,
            &mutation(SecretAction::Replace, Some("password")),
            &passphrase(SecretAction::Keep, None),
        )
        .unwrap();
        assert!(
            store
                .read("target-a", CredentialKind::Passphrase)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn secret_mutations_reject_ambiguous_or_oversized_values() {
        assert!(
            validate_secret_mutations(
                AuthMethod::Password,
                &mutation(SecretAction::Keep, Some("ignored")),
                &passphrase(SecretAction::Keep, None),
            )
            .is_err()
        );
        assert!(
            validate_secret_mutations(
                AuthMethod::Password,
                &mutation(
                    SecretAction::Replace,
                    Some(&"x".repeat(MAX_PASSWORD_BYTES + 1)),
                ),
                &passphrase(SecretAction::Clear, None),
            )
            .is_err()
        );
        assert!(
            validate_secret_mutations(
                AuthMethod::PrivateKey,
                &mutation(SecretAction::Clear, None),
                &passphrase(SecretAction::Replace, Some("orphan")),
            )
            .is_err()
        );
    }

    #[test]
    fn credential_bundle_revision_mismatch_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let store = TerminalSecretStore::new(directory.path());
        apply_credential(
            &store,
            "target-a",
            AuthMethod::Password,
            2,
            None,
            &mutation(SecretAction::Replace, Some("new-secret")),
            &passphrase(SecretAction::Clear, None),
        )
        .unwrap();
        let metadata = TargetRecord {
            id: "target-a".to_string(),
            name: "target".to_string(),
            host: "127.0.0.1".to_string(),
            port: 22,
            username: "operator".to_string(),
            auth_method: AuthMethod::Password,
            trusted_host_key: None,
            revision: 1,
            last_verified_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let decorated = decorate_target(metadata.clone(), &store).unwrap();
        assert!(!decorated.credential_configured);
        assert!(stored_credential(&store, &metadata).is_err());
        assert!(
            apply_credential(
                &store,
                "target-a",
                AuthMethod::Password,
                2,
                Some(&metadata),
                &mutation(SecretAction::Keep, None),
                &passphrase(SecretAction::Clear, None),
            )
            .is_err(),
            "a keep update must not bless a crash-mismatched bundle"
        );
    }

    #[tokio::test]
    async fn active_session_conflict_preserves_verification_for_confirmed_retry() {
        let (_directory, state) = test_state().await;
        let id = Uuid::new_v4().to_string();
        let now = now_iso();
        let existing = TargetRecord {
            id: id.clone(),
            name: "target".to_string(),
            host: "127.0.0.1".to_string(),
            port: 22,
            username: "operator".to_string(),
            auth_method: AuthMethod::Password,
            trusted_host_key: None,
            revision: 1,
            last_verified_at: None,
            created_at: now.clone(),
            updated_at: now,
        };
        TargetRepository::new(&state)
            .insert(existing.clone())
            .await
            .unwrap();
        let secrets = TerminalSecretStore::from_state(&state);
        apply_credential(
            &secrets,
            &id,
            AuthMethod::Password,
            1,
            None,
            &mutation(SecretAction::Replace, Some("secret")),
            &passphrase(SecretAction::Clear, None),
        )
        .unwrap();
        state
            .terminal
            .reserve_active_test_session(&id)
            .await
            .unwrap();

        let mut prospective = existing.clone();
        prospective.host = "127.0.0.2".to_string();
        prospective.revision = 2;
        let fingerprint = connection_verification_fingerprint(
            Some(&id),
            &prospective,
            &SshCredential::Password("secret".to_string()),
        );
        let verification_token = state.terminal.issue_verification(fingerprint).await;
        let input = TargetUpdateInput {
            name: prospective.name.clone(),
            host: prospective.host.clone(),
            port: prospective.port,
            username: prospective.username.clone(),
            auth_method: prospective.auth_method,
            trusted_host_key: None,
            revision: 1,
            credential: mutation(SecretAction::Keep, None),
            passphrase: passphrase(SecretAction::Keep, None),
            verification_token: Some(verification_token),
        };
        let conflict = update_target(&state, &id, input.clone(), false, None)
            .await
            .unwrap_err();
        assert_eq!(conflict.code, TerminalErrorCode::Conflict);
        assert_eq!(conflict.active_session_count, Some(1));
        let confirmation_token = conflict.confirmation_token.unwrap();

        let updated = update_target(&state, &id, input, true, Some(&confirmation_token))
            .await
            .unwrap();
        assert_eq!(updated.revision, 2);
        assert!(updated.last_verified_at.is_some());
        assert_eq!(state.terminal.active_counts(Some(&id)).await, 0);
        state.terminal.shutdown_all().await;
    }
}
