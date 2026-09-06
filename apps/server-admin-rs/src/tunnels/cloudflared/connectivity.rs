use std::collections::HashSet;

use crate::tunnels::connectivity::TunnelConnectivityGate;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CloudflaredSignal {
    Registered(u8),
    Disconnected(u8),
    ProcessExited,
    Reconcile,
}

#[derive(Default)]
pub(super) struct CloudflaredConnectivity {
    pub(super) gate: TunnelConnectivityGate,
    active: HashSet<u8>,
    stopping: bool,
    resume_connected: bool,
}

impl CloudflaredConnectivity {
    // A failed HA connection does not imply the connector is offline. Only
    // transitions involving known connections can change aggregate health.
    pub(super) fn observe_signal(&mut self, signal: CloudflaredSignal) -> Option<bool> {
        let result = match signal {
            CloudflaredSignal::Registered(index) => {
                self.active.insert(index);
                Some(true)
            }
            CloudflaredSignal::Disconnected(index) => {
                (self.active.remove(&index) && self.active.is_empty()).then_some(false)
            }
            CloudflaredSignal::ProcessExited => {
                self.active.clear();
                Some(false)
            }
            CloudflaredSignal::Reconcile => Some(!self.active.is_empty()),
        };
        // Keep tracking output during termination: termination can fail while
        // the same process and its remaining connections continue running.
        if self.stopping { None } else { result }
    }

    pub(super) fn set_expected_stop(&mut self, expected: bool) {
        if expected && !self.stopping {
            self.resume_connected = self.gate.has_connection_baseline();
        }
        self.stopping = expected;
        self.gate.set_expected_stop(expected);
    }

    pub(super) fn finish_expected_stop(&mut self, stopped: bool) {
        self.set_expected_stop(false);
        if stopped {
            self.active.clear();
            self.resume_connected = false;
        } else if self.resume_connected {
            // Restore the notification baseline without publishing a spurious
            // recovery. Reconcile then confirms any outage observed during stop.
            self.gate.observe_connected(tokio::time::Instant::now());
        }
    }
}

pub(super) fn parse_cloudflared_signal(line: &str) -> Option<CloudflaredSignal> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
        let index = value.get("connIndex")?;
        let index = index
            .as_u64()
            .and_then(|v| u8::try_from(v).ok())
            .or_else(|| index.as_str()?.parse().ok())?;
        return classify_message(value.get("message")?.as_str()?, index);
    }

    let tokens = console_tokens(line)?;
    let index = tokens
        .iter()
        .find_map(|token| token.strip_prefix("connIndex=")?.parse::<u8>().ok())?;
    let start = tokens
        .iter()
        .take(2)
        .position(|token| {
            matches!(
                *token,
                "INF" | "WRN" | "ERR" | "DBG" | "TRC" | "FTL" | "PNC"
            )
        })
        .map_or(0, |position| position + 1);
    let message = tokens[start..]
        .iter()
        .take_while(|token| !token.contains('='))
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    classify_message(&message, index)
}

// Zerolog console fields may contain quoted whitespace and escaped quotes.
// Split only outside those strings so error details cannot impersonate fields.
fn console_tokens(line: &str) -> Option<Vec<&str>> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut quoted = false;
    let mut escaped = false;
    for (offset, ch) in line.char_indices() {
        if ch.is_whitespace() && !quoted {
            if let Some(begin) = start.take() {
                tokens.push(&line[begin..offset]);
            }
            continue;
        }
        start.get_or_insert(offset);
        if escaped {
            escaped = false;
        } else if quoted && ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        }
    }
    if quoted {
        return None;
    }
    if let Some(begin) = start {
        tokens.push(&line[begin..]);
    }
    Some(tokens)
}

fn classify_message(message: &str, index: u8) -> Option<CloudflaredSignal> {
    let normalized = message.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "registered tunnel connection" => Some(CloudflaredSignal::Registered(index)),
        "unregistered tunnel connection"
        | "serve tunnel error"
        | "tunnel disconnected"
        | "failed to serve tunnel"
        | "failed to serve tunnel connection"
        | "connection terminated"
        | "retrying connection" => Some(CloudflaredSignal::Disconnected(index)),
        _ => {
            let words: Vec<_> = normalized.split_whitespace().collect();
            // Older cloudflared releases used "Connection <UUID> registered".
            if matches!(words.as_slice(), ["connection", _, "registered"]) {
                Some(CloudflaredSignal::Registered(index))
            } else if normalized.starts_with("retrying connection in ") {
                Some(CloudflaredSignal::Disconnected(index))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnels::connectivity::{ConnectedEventAction, TunnelDisconnectEvent};
    use std::time::Duration;
    use tokio::time::Instant;

    #[test]
    fn parses_issue_53_and_registration_without_confusing_unregistration() {
        for (line, signal) in [
            (
                "2026-09-05T14:34:10Z WRN failed to serve tunnel connection error=\"accept stream listener encountered a failure while serving\" connIndex=2 event=0 ip=2606:4700:a0::9",
                CloudflaredSignal::Disconnected(2),
            ),
            (
                "2026-09-05T14:34:10Z INF Registered tunnel connection connIndex=2 connection=uuid",
                CloudflaredSignal::Registered(2),
            ),
            (
                "INF Unregistered tunnel connection connIndex=2",
                CloudflaredSignal::Disconnected(2),
            ),
            (
                "INF Connection uuid registered connIndex=0",
                CloudflaredSignal::Registered(0),
            ),
            (
                "ERR Serve tunnel error error=\"timeout\" connIndex=1",
                CloudflaredSignal::Disconnected(1),
            ),
            (
                "INF Retrying connection in up to 1m4s connIndex=3",
                CloudflaredSignal::Disconnected(3),
            ),
            (
                r#"{"level":"info","message":"Registered tunnel connection","connIndex":2}"#,
                CloudflaredSignal::Registered(2),
            ),
            (
                r#"{"message":"Unregistered tunnel connection","connIndex":"2"}"#,
                CloudflaredSignal::Disconnected(2),
            ),
        ] {
            assert_eq!(parse_cloudflared_signal(line), Some(signal), "{line}");
        }
    }

    #[test]
    fn ignores_unknown_or_ambiguous_messages() {
        for line in [
            "INF Registered tunnel connection",
            "INF Registered tunnel connection connIndex=bad",
            "INF Registered tunnel connection connIndex=256",
            "ERR unrelated error=\"registered tunnel connection\" connIndex=0",
            "INF Registering tunnel connection connIndex=0",
            r#"{"message":"Registered tunnel connection","connIndex":-1}"#,
        ] {
            assert_eq!(parse_cloudflared_signal(line), None, "{line}");
        }
    }

    #[test]
    fn quoted_error_fields_cannot_supply_or_override_connection_index() {
        for line in [
            r#"ERR Serve tunnel error error="upstream connIndex=0 failed" connIndex=2"#,
            r#"ERR Serve tunnel error error="upstream \"quoted\" connIndex=0 failed" connIndex=2"#,
            r#"ERR Serve tunnel error error="upstream \\ connIndex=0 failed" connIndex=2"#,
        ] {
            assert_eq!(
                parse_cloudflared_signal(line),
                Some(CloudflaredSignal::Disconnected(2)),
                "{line}"
            );
        }
        for line in [
            r#"ERR Serve tunnel error error="upstream connIndex=0 failed""#,
            r#"ERR Serve tunnel error error="truncated connIndex=0"#,
        ] {
            assert_eq!(parse_cloudflared_signal(line), None, "{line}");
        }
    }

    #[test]
    fn failed_stop_preserves_connections_and_detects_later_outage() {
        let mut state = CloudflaredConnectivity::default();
        let now = Instant::now();
        for index in 0..4 {
            state.observe_signal(CloudflaredSignal::Registered(index));
        }
        state.gate.observe_connected(now);
        state.set_expected_stop(true);
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Disconnected(2)),
            None
        );
        state.finish_expected_stop(false);
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Reconcile),
            Some(true)
        );
        assert_eq!(
            state.gate.observe_connected(now),
            ConnectedEventAction::Ignore
        );
        for index in [0, 1] {
            assert_eq!(
                state.observe_signal(CloudflaredSignal::Disconnected(index)),
                None
            );
        }
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Disconnected(3)),
            Some(false)
        );
        assert!(
            state
                .gate
                .observe_disconnected(now, disconnect_event())
                .is_some()
        );
    }

    #[test]
    fn failed_stop_rechecks_outage_that_started_during_termination() {
        let mut state = CloudflaredConnectivity::default();
        let now = Instant::now();
        state.observe_signal(CloudflaredSignal::Registered(0));
        state.gate.observe_connected(now);
        state.set_expected_stop(true);
        state.observe_signal(CloudflaredSignal::Disconnected(0));
        state.finish_expected_stop(false);
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Reconcile),
            Some(false)
        );
        let timer = state
            .gate
            .observe_disconnected(now, disconnect_event())
            .unwrap();
        assert!(
            state
                .gate
                .confirm_disconnect(&timer, timer.deadline)
                .is_some()
        );
    }

    #[test]
    fn failed_stop_does_not_invent_a_connection_for_unconnected_process() {
        let mut state = CloudflaredConnectivity::default();
        state.set_expected_stop(true);
        state.finish_expected_stop(false);
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Reconcile),
            Some(false)
        );
        assert_eq!(
            state.gate.observe_connected(Instant::now()),
            ConnectedEventAction::PublishConnected
        );
    }

    #[test]
    fn failed_stop_does_not_repeat_an_already_confirmed_outage() {
        let mut state = CloudflaredConnectivity::default();
        let now = Instant::now();
        state.observe_signal(CloudflaredSignal::Registered(0));
        state.gate.observe_connected(now);
        state.observe_signal(CloudflaredSignal::Disconnected(0));
        let timer = state
            .gate
            .observe_disconnected(now, disconnect_event())
            .unwrap();
        assert!(
            state
                .gate
                .confirm_disconnect(&timer, timer.deadline)
                .is_some()
        );
        state.set_expected_stop(true);
        state.finish_expected_stop(false);
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Reconcile),
            Some(false)
        );
        assert!(
            state
                .gate
                .observe_disconnected(timer.deadline, disconnect_event())
                .is_none()
        );
    }

    #[test]
    fn parses_index_after_long_error_before_display_truncation() {
        let line = format!(
            "ERR failed to serve tunnel connection error=\"{}\" connIndex=2",
            "x".repeat(500)
        );
        assert_eq!(
            parse_cloudflared_signal(&line),
            Some(CloudflaredSignal::Disconnected(2))
        );
    }

    fn disconnect_event() -> TunnelDisconnectEvent {
        TunnelDisconnectEvent {
            happened_at: "2026-09-05T14:34:10Z".into(),
            message: None,
            pid: Some(42),
        }
    }

    #[test]
    fn partial_failure_never_starts_disconnect_timer() {
        let mut state = CloudflaredConnectivity::default();
        let now = Instant::now();
        for index in 0..4 {
            assert_eq!(
                state.observe_signal(CloudflaredSignal::Registered(index)),
                Some(true)
            );
            state.gate.observe_connected(now);
        }
        for index in [2, 2, 7, 0, 1] {
            assert_eq!(
                state.observe_signal(CloudflaredSignal::Disconnected(index)),
                None
            );
        }
        assert_eq!(
            state.gate.observe_connected(now + Duration::from_secs(120)),
            ConnectedEventAction::Ignore
        );
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Disconnected(3)),
            Some(false)
        );
        let timer = state
            .gate
            .observe_disconnected(now, disconnect_event())
            .unwrap();
        assert!(
            state
                .gate
                .confirm_disconnect(&timer, timer.deadline)
                .is_some()
        );
        assert!(
            state
                .gate
                .confirm_disconnect(&timer, timer.deadline)
                .is_none()
        );
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Registered(2)),
            Some(true)
        );
        assert_eq!(
            state.gate.observe_connected(timer.deadline),
            ConnectedEventAction::PublishConnected
        );
    }

    #[test]
    fn any_connection_recovering_cancels_pending_disconnect() {
        let mut state = CloudflaredConnectivity::default();
        let now = Instant::now();
        state.observe_signal(CloudflaredSignal::Registered(2));
        state.gate.observe_connected(now);
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Disconnected(2)),
            Some(false)
        );
        let timer = state
            .gate
            .observe_disconnected(now, disconnect_event())
            .unwrap();
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Registered(0)),
            Some(true)
        );
        assert_eq!(
            state.gate.observe_connected(now + Duration::from_secs(1)),
            ConnectedEventAction::Ignore
        );
        assert!(
            state
                .gate
                .confirm_disconnect(&timer, timer.deadline)
                .is_none()
        );
    }

    #[test]
    fn process_exit_clears_all_connections_before_restart() {
        let mut state = CloudflaredConnectivity::default();
        state.observe_signal(CloudflaredSignal::Registered(0));
        state.observe_signal(CloudflaredSignal::Registered(1));
        assert_eq!(
            state.observe_signal(CloudflaredSignal::ProcessExited),
            Some(false)
        );
        state.observe_signal(CloudflaredSignal::Registered(2));
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Disconnected(2)),
            Some(false)
        );
    }

    #[test]
    fn expected_stop_cancels_alert_and_ignores_shutdown_output() {
        let mut state = CloudflaredConnectivity::default();
        let now = Instant::now();
        state.observe_signal(CloudflaredSignal::Registered(0));
        state.gate.observe_connected(now);
        state.observe_signal(CloudflaredSignal::Disconnected(0));
        let timer = state
            .gate
            .observe_disconnected(now, disconnect_event())
            .unwrap();
        state.set_expected_stop(true);
        assert_eq!(state.observe_signal(CloudflaredSignal::Registered(1)), None);
        assert!(
            state
                .gate
                .confirm_disconnect(&timer, timer.deadline)
                .is_none()
        );
        state.finish_expected_stop(true);
        state.observe_signal(CloudflaredSignal::Registered(2));
        assert_eq!(
            state.observe_signal(CloudflaredSignal::Disconnected(2)),
            Some(false)
        );
    }
}
