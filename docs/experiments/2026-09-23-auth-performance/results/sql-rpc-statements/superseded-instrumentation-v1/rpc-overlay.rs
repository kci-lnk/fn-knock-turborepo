//! One in-process AuthorizeHttp handler, test-only SQL diagnostic.
//! This excludes Go, gRPC transport/queues, and separately issued Inspect RPCs.
use super::*;
use crate::storage::redis_compat::{
    execute_command_in_transaction, sql_statement_diagnostic as trace,
};

const HOST: &str = "sql-diagnostic.example.com";
const CLIENT_IP: &str = "203.0.113.42";
const SESSION_ID: &str = "diagnostic-session-0000";

async fn request_scope<T>(state: &AppState, future: impl std::future::Future<Output = T>) -> T {
    // Replaced by the driver only when the pinned revision has request_context.
    let _ = state;
    future.await
}

fn grant_key(index: usize) -> String {
    format!(
        "fn_knock:auth:subdomain_rule_grant:{}",
        crate::crypto_utils::sha256_hex_str(&format!("diagnostic-grant-{index}"))
    )
}

fn seed(state: &AppState, population: usize, scenario: &str) -> anyhow::Result<()> {
    let mut connection = tokio_rusqlite::rusqlite::Connection::open(state.storage.store.path())?;
    let tx = connection.transaction()?;
    let now = time_utils::now_ms().div_euclid(1_000);
    let active_key = format!(
        "fn_knock:auth:subdomain_rule_grant_active:{}",
        crate::crypto_utils::sha256_hex_str(HOST)
    );
    let mut credentials = Vec::new();
    for index in 0..population {
        let id = format!("diagnostic-session-{index:04}");
        let credential_id = format!("diagnostic-totp-{index:04}");
        let ip = if index == 0 {
            CLIENT_IP.to_string()
        } else {
            format!("198.18.{}.{}", index / 250, index % 250 + 1)
        };
        credentials.push(
            json!({"id": credential_id, "secret": "diagnostic-only", "comment": "SQL fixture",
            "createdAt": time_utils::now_iso(), "access_scopes": null,
            "subdomain_access": {"mode": "custom", "hosts": [HOST]}}),
        );
        let mut session = crate::store::new_login_session(
            &credential_id,
            "SQL fixture",
            &ip,
            "SQL diagnostic",
            3_600,
        );
        session.ip_location = Some("Diagnostic location".to_string());
        if scenario == "auto_ip_hit" {
            session.grant_type = Some("login_ip_grant".to_string());
            session.post_login_ip_grant_mode = Some("follow_session".to_string());
        }
        execute_command_in_transaction(
            &tx,
            "SET",
            vec![
                crate::auth_session_keys::session_key(&id),
                serde_json::to_string(&session)?,
                "EX".into(),
                "3600".into(),
            ],
        )?;
        execute_command_in_transaction(
            &tx,
            "ZADD",
            vec![
                crate::auth_mobility_keys::active_ip_zset_key(&id),
                now.to_string(),
                ip.clone(),
            ],
        )?;
        execute_command_in_transaction(&tx, "HSET", vec![crate::auth_mobility_keys::active_ip_details_key(&id), ip.clone(), json!({"ip": ip, "firstSeenAt": now, "lastSeenAt": now, "ipLocation": "Diagnostic location"}).to_string()])?;
        let key = grant_key(index);
        let document =
            json!({"host": HOST, "policy_version": "diagnostic-v1", "group_id": "diagnostic-group",
            "issued_at": now, "last_access_at": now, "hard_expires_at": now + 3_600})
            .to_string();
        execute_command_in_transaction(
            &tx,
            "SET",
            vec![key.clone(), document, "EX".into(), "3600".into()],
        )?;
        execute_command_in_transaction(
            &tx,
            "ZADD",
            vec![active_key.clone(), (now + 3_600).to_string(), key],
        )?;
    }
    execute_command_in_transaction(&tx, "EXPIRE", vec![active_key, "3600".into()])?;
    execute_command_in_transaction(
        &tx,
        "SET",
        vec![
            "fn_knock:totps".into(),
            serde_json::to_string(&credentials)?,
        ],
    )?;
    if scenario == "auto_ip_hit" {
        let record = json!({"id": "diagnostic-auto", "ip": CLIENT_IP, "targetType": "ip",
            "expireAt": now + 3600, "source": "auto", "createdAt": now, "status": "active"});
        execute_command_in_transaction(
            &tx,
            "HSET",
            vec![
                "fn_knock:whitelist:records".into(),
                "diagnostic-auto".into(),
                record.to_string(),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

use crate::grpc_proto::{
    AuthBridgeEnvelope, AuthContext, AuthGrantKind, AuthorizeHttpRequest, HttpAuthMode,
    auth_bridge_envelope,
};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Observation only: the bridge conversion records the actual computed access
// source. No authorization outcome is changed by this test-only callback.
static GRANT_TYPES: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub(super) fn record_grant_type(value: Option<&str>) {
    GRANT_TYPES
        .lock()
        .unwrap()
        .push(value.unwrap_or("<none>").to_string());
}

async fn operation(state: &AppState, scenario: &str) -> anyhow::Result<Value> {
    GRANT_TYPES.lock().unwrap().clear();
    let cookie = match scenario {
        "session_hit" => format!("{}={SESSION_ID}", cookies::SESSION_COOKIE_NAME),
        "grant_hit" => "fn-knock-subdomain-rule-grant=diagnostic-grant-0".to_string(),
        "auto_ip_hit" => String::new(),
        _ => anyhow::bail!("unknown scenario"),
    };
    let message = AuthBridgeEnvelope {
        request_id: "sql-diagnostic-request".to_string(),
        deadline_unix_millis: 0,
        payload: Some(auth_bridge_envelope::Payload::AuthorizeHttpRequest(
            AuthorizeHttpRequest {
                context: Some(AuthContext {
                    client_ip: CLIENT_IP.to_string(),
                    forwarded_host: HOST.to_string(),
                    forwarded_proto: "https".to_string(),
                    path: "/".to_string(),
                    access_mode: "login_first".to_string(),
                    user_agent: "SQL diagnostic".to_string(),
                    cookie,
                    ..Default::default()
                }),
                matched: false,
                mode: HttpAuthMode::PreflightAndVerify as i32,
                subdomain_rule_match: None,
            },
        )),
    };
    // Exactly one final-version request_context scope, same placement as the
    // production bridge worker; the baseline has no such scope.
    let envelope = request_scope(
        state,
        super::bridge::sql_diagnostic_handle_bridge_message(state.clone(), message),
    )
    .await
    .ok_or_else(|| anyhow::anyhow!("missing response envelope"))?;
    anyhow::ensure!(
        envelope.request_id == "sql-diagnostic-request",
        "wrong response id"
    );
    let Some(auth_bridge_envelope::Payload::AuthorizeHttpResponse(response)) = envelope.payload
    else {
        anyhow::bail!("wrong response payload")
    };
    let preflight = response
        .preflight
        .ok_or_else(|| anyhow::anyhow!("missing preflight"))?;
    anyhow::ensure!(
        !preflight.deny
            && preflight.redirect_location.is_empty()
            && preflight.access_denied_reason.is_empty(),
        "preflight rejected: {preflight:?}"
    );
    let verify = response
        .verify
        .ok_or_else(|| anyhow::anyhow!("missing verify"))?;
    anyhow::ensure!(
        verify.success && verify.host_authorized && verify.status == 200,
        "verify rejected: {verify:?}"
    );
    let expected = match scenario {
        "session_hit" => "browser_session",
        "auto_ip_hit" => "login_ip_grant",
        _ => "subdomain_rule",
    };
    let observed = GRANT_TYPES.lock().unwrap().clone();
    anyhow::ensure!(
        observed == vec![expected],
        "wrong access source: {observed:?}"
    );
    let kind = if scenario == "grant_hit" {
        AuthGrantKind::SubdomainRule
    } else {
        AuthGrantKind::Login
    };
    anyhow::ensure!(
        verify.grant_kind == kind as i32,
        "wrong grant kind: {verify:?}"
    );
    anyhow::ensure!(
        verify.set_cookies.is_empty(),
        "healthy request must not issue or renew cookies"
    );
    if scenario == "grant_hit" {
        anyhow::ensure!(
            verify.auth_rule_group_id == "diagnostic-group"
                && verify.decision == "subdomain_rule_allowed",
            "wrong rule grant: {verify:?}"
        );
    }
    Ok(
        json!({"request_id": "sql-diagnostic-request", "preflight_denied": false,
        "verify_success": verify.success, "status": verify.status, "grant_type": observed[0],
        "grant_kind": verify.grant_kind, "decision": verify.decision,
        "rule_group_id": verify.auth_rule_group_id, "set_cookie_count": verify.set_cookies.len()}),
    )
}

async fn prepare_fixture(
    state: &AppState,
    population: usize,
    scenario: &str,
) -> anyhow::Result<()> {
    let mut config = state.storage.store.get_config().await?;
    config["run_type"] = json!(3);
    config["auto_manage_firewall"] = json!(false);
    config["auth_credential_settings"] = json!({"post_login_ip_grant_mode": "disabled",
        "session_ip_mobility_enabled": false, "session_ip_mobility_window_seconds": 1200});
    let mut mapping = json!({"host": HOST, "use_auth": true,
        "target": "http://127.0.0.1:28081", "target_type": "proxy"});
    if scenario == "grant_hit" {
        mapping["advanced_auth"] = json!({"enabled": true, "policy_version": "diagnostic-v1",
            "idle_ttl_seconds": 3600, "max_lifetime_seconds": 7200,
            "groups": [{"id": "diagnostic-group", "conditions": [{"id": "issue",
                "target": "request_header", "operator": "equals", "name": "X-Diagnostic-Issue",
                "values": ["allow"]}]}]});
    }
    config["host_mappings"] = json!([mapping]);
    state.storage.store.replace_config(&config).await?;
    seed(state, population, scenario)?;
    crate::whitelist::rebuild_whitelist_ipset_snapshots(state).await?;
    // Prove the fixture cannot take private-network/manual-whitelist shortcuts.
    anyhow::ensure!(
        !http_utils::is_private_or_local_ip(CLIENT_IP),
        "private IP fixture"
    );
    let ip = CLIENT_IP.parse()?;
    anyhow::ensure!(
        !crate::whitelist::whitelist_snapshot_contains(state, ip, Some(&["manual"])),
        "manual bypass"
    );
    anyhow::ensure!(
        crate::whitelist::whitelist_snapshot_contains(state, ip, Some(&["auto"]))
            == (scenario == "auto_ip_hit"),
        "wrong auto-whitelist fixture"
    );
    // This is a setup assertion outside SQL measurement, including authoritative
    // owner confirmation and session identity. No measured method counts are summed.
    let owners = auth_mobility::sql_diagnostic_list_active_sessions_by_ip(state, CLIENT_IP).await?;
    anyhow::ensure!(
        owners.len() == 1 && owners[0].0 == SESSION_ID,
        "not the unique selected owner"
    );
    Ok(())
}

async fn settle_background(state: &AppState) -> anyhow::Result<Value> {
    let start = Instant::now();
    loop {
        let (generation, active) = state.background_tasks.sql_diagnostic_snapshot();
        if active.is_empty() {
            trace::barrier().await;
            return Ok(json!({"spawn_generation": generation, "active": active,
                "waited_ms": start.elapsed().as_millis()}));
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(20),
            "background did not settle: {active:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn collect(state: &AppState, population: usize, scenario: &str) -> anyhow::Result<Value> {
    prepare_fixture(state, population, scenario).await?;
    let warm_started = Instant::now();
    let warm_outcome = operation(state, scenario).await?;
    // Successful verify schedules recent-IP bookkeeping and a 5s debounced
    // rebuild. Wait for actual registry completion rather than guessing an idle
    // sleep, then remain inside its 30s per-IP write-coalescing interval.
    let warm_background = settle_background(state).await?;
    trace::start();
    tokio::time::sleep(Duration::from_millis(20)).await;
    trace::barrier().await;
    let idle = trace::stop();
    anyhow::ensure!(
        idle["trace_callbacks"] == 0,
        "background idle trace: {idle}"
    );
    let before = state.background_tasks.sql_diagnostic_snapshot();
    anyhow::ensure!(
        before.1.is_empty() && warm_started.elapsed() < Duration::from_secs(25),
        "not inside settled warm recent-IP interval"
    );
    trace::start();
    let outcome = operation(state, scenario).await;
    let counts = trace::stop();
    let after = state.background_tasks.sql_diagnostic_snapshot();
    println!(
        "RPC diagnostic {scenario} N={population}: outcome={outcome:?} counts={counts} background={before:?}->{after:?}"
    );
    let outcome = outcome?;
    anyhow::ensure!(
        before == after && after.1.is_empty(),
        "new background work during request"
    );
    anyhow::ensure!(
        counts["statement_executions"].as_u64().unwrap_or(0) > 0,
        "empty trace"
    );
    anyhow::ensure!(
        counts["by_category"]["trigger_subprogram"].is_null(),
        "unexpected trigger"
    );
    // A second idle capture detects queued synchronous work after handler return.
    trace::barrier().await;
    trace::start();
    tokio::time::sleep(Duration::from_millis(20)).await;
    trace::barrier().await;
    let idle_after = trace::stop();
    anyhow::ensure!(
        idle_after["trace_callbacks"] == 0,
        "post-handler background trace: {idle_after}"
    );
    Ok(
        json!({"population": population, "operation": scenario, "mode": "PreflightAndVerify",
        "outcome": outcome, "counts": counts, "idle_control": idle, "idle_after": idle_after,
        "warm_outcome": warm_outcome, "warm_background": warm_background,
        "background_generation_before": before.0, "background_generation_after": after.0,
        "warm_age_ms": warm_started.elapsed().as_millis()}),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn operation_statement_counts() {
    let output = std::env::var("FN_KNOCK_SQL_DIAGNOSTIC_OUT").expect("diagnostic output path");
    let mut rows = Vec::new();
    let mut calibrations = Vec::new();
    for population in [1, 100, 1000] {
        for scenario in ["session_hit", "grant_hit", "auto_ip_hit"] {
            let (_directory, state) =
                super::tests::auth_route_test_state("sql-rpc-statements").await;
            calibrations.push(json!({"population": population, "scenario": scenario, "counts": trace::calibration().await}));
            let result = collect(&state, population, scenario).await;
            trace::unregister()
                .await
                .expect("unregister before fixture drop");
            let unfinished = state
                .shutdown_background_tasks(Duration::from_secs(20))
                .await;
            assert!(
                unfinished.is_empty(),
                "unfinished fixture work: {unfinished:?}"
            );
            rows.push(result.expect("SQL diagnostic complete AuthorizeHttp handler"));
        }
    }
    let report = json!({"schema_version": 1,
        "boundary": "one in-process AuthorizeHttp complete Rust handler in PreflightAndVerify mode; excludes Go, transport, admission/send queues, and separate Inspect RPC",
        "background_semantics": "warmed request within 30s recent-IP coalescing interval; prior background tasks completed, no task spawned during measured handler; cold/periodic background SQL not included",
        "source_commit": std::env::var("FN_KNOCK_SQL_DIAGNOSTIC_SOURCE").unwrap(),
        "auth_readers": 1, "populations": [1, 100, 1000], "calibrations": calibrations, "measurements": rows});
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
