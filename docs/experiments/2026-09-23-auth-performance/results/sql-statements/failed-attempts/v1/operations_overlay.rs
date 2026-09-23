//! Operation-level SQL counts. Not an HTTP/RPC request tracer or benchmark.
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

fn seed(state: &AppState, population: usize) -> anyhow::Result<()> {
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
    tx.commit()?;
    Ok(())
}

async fn operation(state: &AppState, config: &Value, name: &str) -> anyhow::Result<Value> {
    request_scope(state, async {
        match name {
            "healthy_grant_get" => {
                let raw = state
                    .storage
                    .store
                    .get_string_value_auth(&grant_key(0))
                    .await?;
                anyhow::ensure!(raw.is_some(), "grant must be valid");
                Ok(json!({"present": true}))
            }
            "valid_session_normal_access" => {
                let mut headers = HeaderMap::new();
                headers.insert("x-forwarded-host", HeaderValue::from_static(HOST));
                headers.insert("x-forwarded-uri", HeaderValue::from_static("/"));
                headers.insert(
                    header::COOKIE,
                    HeaderValue::from_str(&format!(
                        "{}={SESSION_ID}",
                        cookies::SESSION_COOKIE_NAME
                    ))?,
                );
                let uri: Uri = "/".parse()?;
                let result = resolve_preflight_normal_access(
                    state,
                    &headers,
                    &uri,
                    config,
                    CLIENT_IP,
                    RequestedAccessMode::LoginFirst,
                )
                .await?;
                anyhow::ensure!(
                    result.authorized && result.grant_type.as_deref() == Some("browser_session"),
                    "expected browser session, got authorized={} grant={:?}",
                    result.authorized,
                    result.grant_type
                );
                Ok(json!({"authorized": true, "grant_type": result.grant_type}))
            }
            "auto_ip_owner_lookup" => {
                let owners = auth_mobility::list_active_sessions_by_ip(state, CLIENT_IP).await?;
                anyhow::ensure!(
                    owners.len() == 1 && owners[0].0 == SESSION_ID,
                    "expected exactly the selected owner"
                );
                Ok(json!({"owner_count": owners.len(), "owner_id": owners[0].0}))
            }
            _ => anyhow::bail!("unknown operation"),
        }
    })
    .await
}

async fn collect(state: &AppState, population: usize) -> anyhow::Result<Vec<Value>> {
    let config = json!({"run_type": 3,
        "subdomain_mode": {"root_domain": "example.com", "auth_host": "auth.example.com", "public_https_port": 443},
        "host_mappings": [{"host": HOST, "use_auth": true}],
        "auth_credential_settings": {"session_ip_mobility_enabled": true, "session_ip_mobility_window_seconds": 1200}});
    state.storage.store.save_config(&config).await?;
    seed(state, population)?;
    let names = [
        "healthy_grant_get",
        "valid_session_normal_access",
        "auto_ip_owner_lookup",
    ];
    // First read repairs raw fixture shadows and warms caches. This work is not
    // part of the measured healthy operation; the fixture is otherwise shared.
    for name in names {
        operation(state, &config, name).await?;
    }
    trace::barrier().await;
    trace::start();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    trace::barrier().await;
    let idle = trace::stop();
    anyhow::ensure!(
        idle["trace_callbacks"] == 0,
        "unexpected background SQL: {idle}"
    );
    let mut rows = Vec::new();
    for name in names {
        trace::barrier().await;
        trace::start();
        let outcome = operation(state, &config, name).await;
        let counts = trace::stop();
        let outcome = outcome?;
        anyhow::ensure!(
            counts["statement_executions"].as_u64().unwrap_or(0) > 0,
            "empty trace"
        );
        anyhow::ensure!(
            counts["by_category"]["trigger_subprogram"].is_null(),
            "fixture unexpectedly has triggers"
        );
        rows.push(json!({"population": population, "operation": name, "outcome": outcome, "counts": counts, "idle_control": idle}));
    }
    Ok(rows)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn operation_statement_counts() {
    let output = std::env::var("FN_KNOCK_SQL_DIAGNOSTIC_OUT").expect("diagnostic output path");
    let mut rows = Vec::new();
    let mut calibrations = Vec::new();
    for population in [1, 100, 1_000] {
        let (_directory, state) = super::tests::auth_route_test_state("sql-statements").await;
        calibrations.push(json!({"population": population, "counts": trace::calibration().await}));
        let result = collect(&state, population).await;
        trace::unregister()
            .await
            .expect("unregister before fixture drop");
        rows.extend(result.expect("SQL diagnostic operation"));
        // Registration has been removed before state/temporary DB destruction.
    }
    let report = json!({"schema_version": 1,
        "boundary": "one awaited method invocation after fixture setup and warmup; not HTTP/RPC SQL per request",
        "source_commit": std::env::var("FN_KNOCK_SQL_DIAGNOSTIC_SOURCE").unwrap(),
        "auth_readers": 1, "populations": [1, 100, 1000], "calibrations": calibrations, "measurements": rows});
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
