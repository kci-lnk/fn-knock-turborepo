use std::{future::Future, time::Duration};

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

const SYNC_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);
const SYNC_RETRY_DELAY: Duration = Duration::from_secs(1);
const SYNC_MAX_RETRY_DELAY: Duration = Duration::from_secs(8);

pub(super) async fn sync_memory(
    state: &AppState,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_operation(
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

pub(super) async fn sync_host_rules(
    state: &AppState,
    config: &Value,
    shutdown: &CancellationToken,
    total_timeout: Duration,
) -> anyhow::Result<()> {
    retry_operation(
        "gateway host rules",
        shutdown,
        total_timeout,
        SYNC_ATTEMPT_TIMEOUT,
        SYNC_RETRY_DELAY,
        |attempt_timeout| {
            crate::proxy_config::sync_go_host_rules_for_config_with_timeout_locked(
                state,
                config,
                attempt_timeout,
            )
        },
    )
    .await
    .map_err(anyhow::Error::msg)
}

async fn retry_operation<F, Fut>(
    operation_name: &'static str,
    shutdown: &CancellationToken,
    total_timeout: Duration,
    attempt_timeout: Duration,
    retry_delay: Duration,
    mut operation: F,
) -> Result<(), String>
where
    F: FnMut(Duration) -> Fut,
    Fut: Future<Output = Result<(), String>>,
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
            Err(error) if !is_transient_error(&error) => return Err(error),
            Err(error) => error,
        };
        let current_retry_delay = retry_delay_for_attempt(retry_delay, attempts);
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining <= current_retry_delay {
            return Err(format!(
                "{operation_name} startup sync failed after {attempts} attempts: {error}"
            ));
        }
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

fn retry_delay_for_attempt(base: Duration, attempts: u32) -> Duration {
    let exponent = attempts.saturating_sub(1).min(3);
    base.saturating_mul(1_u32 << exponent)
        .min(SYNC_MAX_RETRY_DELAY)
}

fn is_transient_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "timeout expired",
        "timed out",
        "deadline exceeded",
        "deadlineexceeded",
        "returned 500 internal server error",
        "returned 502 bad gateway",
        "returned 503 service unavailable",
        "returned 504 gateway timeout",
        "status: unavailable",
        "transport error",
        "connection refused",
        "connection reset",
    ]
    .iter()
    .any(|marker| error.contains(marker))
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
    fn transient_error_classification_is_conservative() {
        for error in [
            "Timeout expired",
            "request timed out",
            "deadline exceeded",
            "returned 500 Internal Server Error",
            "returned 502 Bad Gateway",
            "returned 503 Service Unavailable",
            "returned 504 Gateway Timeout",
            "status: Unavailable",
            "transport error",
            "connection refused",
            "connection reset by peer",
        ] {
            assert!(is_transient_error(error), "expected transient: {error}");
        }
        for error in [
            "returned 400 Bad Request",
            "returned 401 Unauthorized",
            "invalid host rule",
        ] {
            assert!(!is_transient_error(error), "expected permanent: {error}");
        }
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
                Duration::from_secs(8),
                Duration::from_secs(8),
                Duration::from_secs(8),
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
