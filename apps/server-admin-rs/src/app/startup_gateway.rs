use std::{future::Future, time::Duration};

use serde_json::{Map, json};
use tokio_util::sync::CancellationToken;

use crate::{state::AppState, transient_error::is_transient_runtime_error};

const SYNC_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);
const SYNC_RETRY_DELAY: Duration = Duration::from_secs(1);
const SYNC_MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

pub(super) async fn sync_memory(
    state: &AppState,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_state_operation(
        state,
        "gateway memory configuration",
        shutdown,
        total_timeout,
        SYNC_ATTEMPT_TIMEOUT,
        SYNC_RETRY_DELAY,
        |attempt_timeout| async move {
            crate::gateway_settings::sync_gateway_memory_on_boot_with_timeout(
                state,
                attempt_timeout,
            )
            .await
            .map(|_| ())
            .map_err(|error| format!("{error:#}"))
        },
    )
    .await
    .map_err(anyhow::Error::msg)
}

pub(super) async fn sync_current_host_rules(
    state: &AppState,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_state_operation(
        state,
        "gateway host rules",
        shutdown,
        total_timeout,
        SYNC_ATTEMPT_TIMEOUT,
        SYNC_RETRY_DELAY,
        |attempt_timeout| async move {
            let config = state
                .storage
                .store
                .get_config()
                .await
                .map_err(|error| error.to_string())?;
            crate::proxy_config::sync_go_host_rules_for_config_with_timeout_locked(
                state,
                &config,
                attempt_timeout,
            )
            .await
        },
    )
    .await
    .map_err(anyhow::Error::msg)
}

pub(super) async fn migrate_visibility_policies(
    state: &AppState,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_state_operation(
        state,
        "gateway visibility policy migration",
        shutdown,
        total_timeout,
        SYNC_ATTEMPT_TIMEOUT,
        SYNC_RETRY_DELAY,
        |_| async move {
            crate::gateway_settings::migrate_visibility_policies_on_boot(state)
                .await
                .map_err(|error| format!("{error:#}"))
        },
    )
    .await
    .map_err(anyhow::Error::msg)
}

pub(super) async fn migrate_common_auth_locations(
    state: &AppState,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_state_operation(
        state,
        "common authentication locations",
        shutdown,
        total_timeout,
        SYNC_ATTEMPT_TIMEOUT,
        SYNC_RETRY_DELAY,
        |_| async move {
            crate::common_auth_locations::migrate_common_auth_location_ipset_on_boot(state)
                .await
                .map_err(|error| format!("{error:#}"))
        },
    )
    .await
    .map_err(anyhow::Error::msg)
}

pub(super) async fn migrate_whitelist_runtime(
    state: &AppState,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_state_operation(
        state,
        "whitelist gateway runtime",
        shutdown,
        total_timeout,
        SYNC_ATTEMPT_TIMEOUT,
        SYNC_RETRY_DELAY,
        |_| async move {
            crate::whitelist::migrate_whitelist_ipsets_on_boot(state)
                .await
                .map_err(|error| format!("{error:#}"))
        },
    )
    .await
    .map_err(anyhow::Error::msg)
}

pub(super) async fn sync_boot_runtime(
    state: &AppState,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_state_operation(
        state,
        "application boot runtime",
        shutdown,
        total_timeout,
        SYNC_ATTEMPT_TIMEOUT,
        SYNC_RETRY_DELAY,
        |_| {
            let state = state.clone();
            async move { super::boot::run_boot_sync_tasks(state).await }
        },
    )
    .await
    .map_err(anyhow::Error::msg)
}

async fn retry_state_operation<F, Fut>(
    state: &AppState,
    operation_name: &'static str,
    shutdown: &CancellationToken,
    total_timeout: Duration,
    attempt_timeout: Duration,
    retry_delay: Duration,
    operation: F,
) -> Result<(), String>
where
    F: FnMut(Duration) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    state.runtime_health.operational_log(
        "INFO",
        "startup_sync",
        "apply_started",
        "gateway_sync_pending",
        Map::from_iter([("operation".to_string(), json!(operation_name))]),
    );
    let result = retry_operation_with_observer(
        operation_name,
        shutdown,
        total_timeout,
        attempt_timeout,
        retry_delay,
        |attempt, retry_delay, error| {
            state.runtime_health.operational_log(
                "WARN",
                "startup_sync",
                "retry_scheduled",
                "transient_gateway",
                Map::from_iter([
                    ("operation".to_string(), json!(operation_name)),
                    ("attempt".to_string(), json!(attempt)),
                    ("retry_delay_ms".to_string(), json!(retry_delay.as_millis())),
                    (
                        "failure_class".to_string(),
                        json!(startup_failure_class(error)),
                    ),
                ]),
            );
        },
        operation,
    )
    .await;
    match &result {
        Ok(()) => state.runtime_health.operational_log(
            "INFO",
            "startup_sync",
            "apply_completed",
            "gateway_sync_applied",
            Map::from_iter([("operation".to_string(), json!(operation_name))]),
        ),
        Err(error) => state.runtime_health.operational_log(
            "ERROR",
            "startup_sync",
            "apply_failed",
            "gateway_sync_rejected",
            Map::from_iter([
                ("operation".to_string(), json!(operation_name)),
                (
                    "failure_class".to_string(),
                    json!(startup_failure_class(error)),
                ),
            ]),
        ),
    }
    result
}

#[cfg(test)]
async fn retry_operation<F, Fut>(
    operation_name: &'static str,
    shutdown: &CancellationToken,
    total_timeout: Duration,
    attempt_timeout: Duration,
    retry_delay: Duration,
    operation: F,
) -> Result<(), String>
where
    F: FnMut(Duration) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    retry_operation_with_observer(
        operation_name,
        shutdown,
        total_timeout,
        attempt_timeout,
        retry_delay,
        |_, _, _| {},
        operation,
    )
    .await
}

async fn retry_operation_with_observer<F, Fut, O>(
    operation_name: &'static str,
    shutdown: &CancellationToken,
    total_timeout: Duration,
    attempt_timeout: Duration,
    retry_delay: Duration,
    mut on_retry: O,
    mut operation: F,
) -> Result<(), String>
where
    F: FnMut(Duration) -> Fut,
    Fut: Future<Output = Result<(), String>>,
    O: FnMut(u32, Duration, &str),
{
    let deadline = tokio::time::Instant::now() + total_timeout;
    let mut attempts = 0_u32;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "{operation_name} startup sync timed out after {attempts} attempts"
            ));
        }
        attempts += 1;
        let current_attempt_timeout = attempt_timeout.min(remaining);
        let result = tokio::select! {
            _ = shutdown.cancelled() => return Err("startup cancelled".to_string()),
            result = tokio::time::timeout(
                current_attempt_timeout,
                operation(current_attempt_timeout),
            ) => match result {
                Ok(result) => result,
                Err(_) => Err(format!(
                    "{operation_name} attempt timed out after {:.1} seconds",
                    current_attempt_timeout.as_secs_f64(),
                )),
            },
        };
        let error = match result {
            Ok(()) => return Ok(()),
            Err(error) if !is_transient_runtime_error(&error) => return Err(error),
            Err(error) => error,
        };
        let current_retry_delay = retry_delay_for_attempt(retry_delay, attempts);
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining <= current_retry_delay {
            return Err(format!(
                "{operation_name} startup sync failed after {attempts} attempts: {error}"
            ));
        }
        on_retry(attempts, current_retry_delay, &error);
        tracing::warn!(
            operation = operation_name,
            attempt = attempts,
            retry_delay_ms = current_retry_delay.as_millis(),
            %error,
            "transient gateway startup sync failure; retrying"
        );
        tokio::select! {
            _ = shutdown.cancelled() => return Err("startup cancelled".to_string()),
            _ = tokio::time::sleep(current_retry_delay.min(remaining)) => {}
        }
    }
}

fn startup_failure_class(error: &str) -> &'static str {
    if is_transient_runtime_error(error) {
        "transient_gateway"
    } else if error.eq_ignore_ascii_case("startup cancelled") {
        "cancelled"
    } else {
        "permanent"
    }
}

fn retry_delay_for_attempt(base: Duration, attempts: u32) -> Duration {
    let exponent = attempts.saturating_sub(1).min(5);
    base.saturating_mul(1_u32 << exponent)
        .min(SYNC_MAX_RETRY_DELAY)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    #[tokio::test]
    async fn transient_sync_is_retried() {
        let shutdown = CancellationToken::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let operation_attempts = attempts.clone();

        retry_operation(
            "test operation",
            &shutdown,
            Duration::from_secs(1),
            Duration::from_millis(50),
            Duration::from_millis(1),
            move |_| {
                let attempt = operation_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        Err("set_host_rules returned 502 Bad Gateway: Timeout expired".to_string())
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn gateway_visibility_transport_error_is_retried() {
        let shutdown = CancellationToken::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let operation_attempts = attempts.clone();

        retry_operation(
            "gateway visibility policy migration",
            &shutdown,
            Duration::from_secs(1),
            Duration::from_millis(50),
            Duration::from_millis(1),
            move |_| {
                let attempt = operation_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        Err(
                            "set_gateway_visibility returned 502 Bad Gateway: transport error"
                                .to_string(),
                        )
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn permanent_sync_error_is_not_retried() {
        let shutdown = CancellationToken::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let operation_attempts = attempts.clone();

        let error = retry_operation(
            "test operation",
            &shutdown,
            Duration::from_secs(1),
            Duration::from_millis(50),
            Duration::from_millis(1),
            move |_| {
                operation_attempts.fetch_add(1, Ordering::SeqCst);
                async { Err("set_host_rules returned 400 Bad Request".to_string()) }
            },
        )
        .await
        .unwrap_err();

        assert_eq!(error, "set_host_rules returned 400 Bad Request");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn transient_retry_delay_is_bounded_exponential_backoff() {
        let base = Duration::from_secs(1);
        let delays = (1..=7)
            .map(|attempt| retry_delay_for_attempt(base, attempt))
            .collect::<Vec<_>>();
        assert_eq!(
            delays,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
                Duration::from_secs(16),
                Duration::from_secs(30),
                Duration::from_secs(30),
            ]
        );
    }

    #[tokio::test]
    async fn cancellation_stops_a_pending_startup_sync() {
        let shutdown = CancellationToken::new();
        shutdown.cancel();

        let error = retry_operation(
            "test operation",
            &shutdown,
            Duration::from_secs(1),
            Duration::from_millis(50),
            Duration::from_millis(1),
            |_| std::future::pending(),
        )
        .await
        .unwrap_err();

        assert_eq!(error, "startup cancelled");
    }

    #[tokio::test]
    async fn total_budget_bounds_an_operation_that_never_returns() {
        let shutdown = CancellationToken::new();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            retry_operation(
                "test operation",
                &shutdown,
                Duration::from_millis(20),
                Duration::from_millis(10),
                Duration::from_millis(1),
                |_| std::future::pending(),
            ),
        )
        .await
        .expect("startup sync exceeded its outer test deadline")
        .unwrap_err();

        assert!(result.contains("test operation startup sync"));
    }
}
