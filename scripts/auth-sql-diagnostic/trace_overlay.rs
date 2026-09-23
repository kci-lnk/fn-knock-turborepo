//! Test-only overlay: never compiled into the product binary.
use std::{
    collections::BTreeMap,
    ffi::{CStr, c_int, c_uint, c_void},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use serde_json::{Value, json};
use tokio_rusqlite::{Connection, rusqlite::ffi};

use super::{ConnectionManager, RedisResult};

const ROLES: [&str; 4] = ["primary", "analytics", "auth_read", "health"];
type Histogram = BTreeMap<(String, String, bool, String), u64>;
static COUNTS: Mutex<Option<Histogram>> = Mutex::new(None);
static CONNECTIONS: Mutex<Vec<Connection>> = Mutex::new(Vec::new());
static CALLBACK_FAILED: AtomicBool = AtomicBool::new(false);

// Context is an integer role tag, never a pointer to Rust-owned memory. The
// recorder is static, so cancellation/panic cannot leave a dangling callback.
unsafe extern "C" fn trace(
    event: c_uint,
    context: *mut c_void,
    statement: *mut c_void,
    sql: *mut c_void,
) -> c_int {
    let result = std::panic::catch_unwind(|| {
        if event != ffi::SQLITE_TRACE_STMT as c_uint {
            return;
        }
        let mut recorder = COUNTS.lock().unwrap_or_else(|error| error.into_inner());
        let Some(counts) = recorder.as_mut() else {
            return;
        };
        if sql.is_null() || statement.is_null() {
            CALLBACK_FAILED.store(true, Ordering::Relaxed);
            return;
        }
        // X can be a trigger comment. For normal statements sqlite3_sql returns
        // the original template; never use sqlite3_expanded_sql (bound values).
        let event_sql = unsafe { CStr::from_ptr(sql.cast()) };
        let trigger = event_sql.to_bytes().starts_with(b"--");
        let template_pointer = if trigger {
            sql.cast()
        } else {
            unsafe { ffi::sqlite3_sql(statement.cast()) }
        };
        if template_pointer.is_null() {
            CALLBACK_FAILED.store(true, Ordering::Relaxed);
            return;
        }
        // SQLite owns these pointers for the duration of this callback.
        let sql = unsafe { CStr::from_ptr(template_pointer) }.to_string_lossy();
        let template = sql.split_whitespace().collect::<Vec<_>>().join(" ");
        let readonly = unsafe { ffi::sqlite3_stmt_readonly(statement.cast()) } != 0;
        let first = template
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let category = if trigger {
            "trigger_subprogram"
        } else if matches!(
            first.as_str(),
            "BEGIN" | "COMMIT" | "END" | "ROLLBACK" | "SAVEPOINT" | "RELEASE"
        ) {
            "transaction"
        } else if first == "PRAGMA" {
            "pragma"
        } else if readonly {
            "read"
        } else {
            "write"
        };
        let role = ROLES[(context as usize) - 1].to_string();
        *counts
            .entry((role, category.to_string(), readonly, template))
            .or_default() += 1;
    });
    if result.is_err() {
        CALLBACK_FAILED.store(true, Ordering::Relaxed);
    }
    ffi::SQLITE_OK
}

impl ConnectionManager {
    pub(super) async fn sql_diagnostic_attach(&self) -> RedisResult<()> {
        if std::env::var_os("FN_KNOCK_SQL_DIAGNOSTIC_OUT").is_none() {
            return Ok(());
        }
        assert_eq!(
            std::env::var("FN_KNOCK_SQLITE_AUTH_READERS").as_deref(),
            Ok("1")
        );
        assert!(
            CONNECTIONS.lock().unwrap().is_empty(),
            "one fixture at a time"
        );
        for (index, connection) in [
            &self.db,
            &self.analytics_db,
            &self.auth_read_db,
            &self.health_db,
        ]
        .into_iter()
        .enumerate()
        {
            // Retain before registration, including the current connection in
            // cleanup if the call fails after installing a callback.
            CONNECTIONS.lock().unwrap().push(connection.clone());
            let result = connection
                .call(move |connection| {
                    // Registration occurs on this connection's owning worker. No
                    // handle or statement escapes the worker/callback lifetime.
                    let status = unsafe {
                        ffi::sqlite3_trace_v2(
                            connection.handle(),
                            ffi::SQLITE_TRACE_STMT as c_uint,
                            Some(trace),
                            (index + 1) as *mut c_void,
                        )
                    };
                    if status != ffi::SQLITE_OK {
                        return Err(crate::storage::storage_error(format!(
                            "trace registration failed: {status}"
                        )));
                    }
                    Ok::<_, crate::storage::StorageError>(())
                })
                .await;
            if let Err(error) = result {
                // Clear callbacks already installed on earlier connections. If
                // cleanup fails, retained handles/static callback state remain
                // alive and the diagnostic aborts without accepting a report.
                if let Err(cleanup_error) = unregister().await {
                    return Err(crate::storage::storage_error(format!(
                        "trace registration: {error}; cleanup: {cleanup_error}"
                    )));
                }
                return Err(error.into());
            }
        }
        Ok(())
    }
}

pub(crate) async fn barrier() {
    let connections = CONNECTIONS.lock().unwrap().clone();
    assert_eq!(connections.len(), 4);
    for connection in connections {
        connection
            .call(|_| Ok::<_, crate::storage::StorageError>(()))
            .await
            .unwrap();
    }
}

pub(crate) async fn calibration() -> Value {
    barrier().await;
    start();
    let connections = CONNECTIONS.lock().unwrap().clone();
    for connection in connections {
        connection
            .call(|connection| {
                let tx = connection.transaction()?;
                {
                    let mut statement = tx.prepare("SELECT ?1")?;
                    for value in [1_i64, 2] {
                        let result: i64 = statement.query_row([value], |row| row.get(0))?;
                        assert_eq!(result, value);
                    }
                }
                tx.commit()?;
                Ok::<_, crate::storage::StorageError>(())
            })
            .await
            .unwrap();
    }
    let result = stop();
    assert_eq!(result["statement_executions"], 16);
    assert_eq!(result["by_category"]["read"], 8);
    assert_eq!(result["by_category"]["transaction"], 8);
    for role in ROLES {
        assert_eq!(result["by_connection"][role], 4);
    }
    result
}

pub(crate) fn start() {
    CALLBACK_FAILED.store(false, Ordering::Relaxed);
    let mut recorder = COUNTS.lock().unwrap();
    assert!(recorder.is_none());
    *recorder = Some(BTreeMap::new());
}

pub(crate) fn stop() -> Value {
    let histogram = COUNTS.lock().unwrap().take().expect("active SQL capture");
    assert!(
        !CALLBACK_FAILED.load(Ordering::Relaxed),
        "trace callback failed"
    );
    let mut categories = BTreeMap::<String, u64>::new();
    let mut connections = BTreeMap::<String, u64>::new();
    let mut templates = Vec::new();
    let mut callbacks = 0;
    let mut statements = 0;
    for ((connection, category, readonly, sql), count) in histogram {
        callbacks += count;
        if category != "trigger_subprogram" {
            statements += count;
        }
        *categories.entry(category.clone()).or_default() += count;
        *connections.entry(connection.clone()).or_default() += count;
        templates.push(json!({"connection": connection, "category": category,
            "sqlite_readonly": readonly, "sql_template": sql, "executions": count}));
    }
    json!({"trace_callbacks": callbacks, "statement_executions": statements,
        "by_category": categories, "by_connection": connections, "templates": templates})
}

pub(crate) async fn unregister() -> RedisResult<()> {
    *COUNTS.lock().unwrap() = None;
    let connections = CONNECTIONS.lock().unwrap().clone();
    for connection in connections {
        connection
            .call(|connection| {
                let status = unsafe {
                    ffi::sqlite3_trace_v2(connection.handle(), 0, None, std::ptr::null_mut())
                };
                if status != ffi::SQLITE_OK {
                    return Err(crate::storage::storage_error(format!(
                        "trace unregister failed: {status}"
                    )));
                }
                Ok::<_, crate::storage::StorageError>(())
            })
            .await?;
    }
    // Unregister first, then release the retained connection handles.
    CONNECTIONS.lock().unwrap().clear();
    Ok(())
}
