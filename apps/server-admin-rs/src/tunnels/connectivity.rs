use std::time::Duration;

use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

pub(super) const DISCONNECT_GRACE_PERIOD: Duration = Duration::from_secs(30);

#[derive(Debug, Eq, PartialEq)]
pub(super) struct TunnelDisconnectEvent {
    pub(super) happened_at: String,
    pub(super) message: Option<String>,
    pub(super) pid: Option<u32>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ConnectedEventAction {
    Ignore,
    PublishConnected,
    PublishDisconnectThenConnected(TunnelDisconnectEvent),
}

#[derive(Clone)]
pub(super) struct DisconnectTimer {
    generation: u64,
    pub(super) deadline: Instant,
    cancellation: CancellationToken,
}

impl DisconnectTimer {
    pub(super) async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }
}

struct PendingDisconnect {
    timer: DisconnectTimer,
    event: TunnelDisconnectEvent,
}

#[derive(Default)]
pub(super) struct TunnelConnectivityGate {
    connected: bool,
    stop_requested: bool,
    pending_disconnect: Option<PendingDisconnect>,
    next_generation: u64,
}

impl TunnelConnectivityGate {
    // A pending outage has not yet been reported as disconnected.
    pub(super) fn has_connection_baseline(&self) -> bool {
        self.connected || self.pending_disconnect.is_some()
    }

    pub(super) fn observe_connected(&mut self, now: Instant) -> ConnectedEventAction {
        if self.connected || self.stop_requested {
            return ConnectedEventAction::Ignore;
        }
        self.connected = true;
        let Some(pending) = self.pending_disconnect.take() else {
            return ConnectedEventAction::PublishConnected;
        };
        pending.timer.cancellation.cancel();
        if now < pending.timer.deadline {
            ConnectedEventAction::Ignore
        } else {
            ConnectedEventAction::PublishDisconnectThenConnected(pending.event)
        }
    }

    pub(super) fn observe_disconnected(
        &mut self,
        now: Instant,
        event: TunnelDisconnectEvent,
    ) -> Option<DisconnectTimer> {
        if !self.connected {
            return None;
        }
        self.connected = false;
        if self.stop_requested {
            return None;
        }
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let timer = DisconnectTimer {
            generation: self.next_generation,
            deadline: now + DISCONNECT_GRACE_PERIOD,
            cancellation: CancellationToken::new(),
        };
        self.pending_disconnect = Some(PendingDisconnect {
            timer: timer.clone(),
            event,
        });
        Some(timer)
    }

    pub(super) fn confirm_disconnect(
        &mut self,
        timer: &DisconnectTimer,
        now: Instant,
    ) -> Option<TunnelDisconnectEvent> {
        let pending = self.pending_disconnect.as_ref()?;
        if self.stop_requested
            || self.connected
            || pending.timer.generation != timer.generation
            || now < pending.timer.deadline
        {
            return None;
        }
        self.pending_disconnect.take().map(|pending| pending.event)
    }

    pub(super) fn set_expected_stop(&mut self, expected: bool) {
        self.stop_requested = expected;
        if expected {
            self.connected = false;
            if let Some(pending) = self.pending_disconnect.take() {
                pending.timer.cancellation.cancel();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disconnect_event(message: &str) -> TunnelDisconnectEvent {
        TunnelDisconnectEvent {
            happened_at: "2026-08-06T12:34:56Z".to_string(),
            message: Some(message.to_string()),
            pid: Some(42),
        }
    }

    #[test]
    fn grace_period_is_thirty_seconds() {
        assert_eq!(DISCONNECT_GRACE_PERIOD, Duration::from_secs(30));
    }

    #[test]
    fn publishes_only_the_first_connected_signal() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();

        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );
        assert_eq!(gate.observe_connected(now), ConnectedEventAction::Ignore);
    }

    #[test]
    fn reconnect_before_confirmation_suppresses_both_edges() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();
        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );

        let timer = gate
            .observe_disconnected(now, disconnect_event("first"))
            .unwrap();

        assert_eq!(
            gate.observe_connected(timer.deadline - Duration::from_millis(1)),
            ConnectedEventAction::Ignore
        );
        assert!(gate.confirm_disconnect(&timer, timer.deadline).is_none());
    }

    #[tokio::test]
    async fn reconnect_wakes_the_cancelled_timer_immediately() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();
        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );
        let timer = gate
            .observe_disconnected(now, disconnect_event("cancelled"))
            .unwrap();

        assert_eq!(
            gate.observe_connected(now + Duration::from_secs(1)),
            ConnectedEventAction::Ignore
        );

        tokio::time::timeout(Duration::from_millis(50), timer.cancelled())
            .await
            .expect("cancelled timer should wake without waiting for its deadline");
    }

    #[test]
    fn persistent_disconnect_is_confirmed_once_and_recovery_is_published() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();
        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );

        let timer = gate
            .observe_disconnected(now, disconnect_event("persistent"))
            .unwrap();

        assert_eq!(
            gate.confirm_disconnect(&timer, timer.deadline),
            Some(disconnect_event("persistent"))
        );
        assert!(gate.confirm_disconnect(&timer, timer.deadline).is_none());
        assert_eq!(
            gate.observe_connected(timer.deadline),
            ConnectedEventAction::PublishConnected
        );
    }

    #[test]
    fn reconnect_at_the_deadline_publishes_the_pending_disconnect_first() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();
        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );
        let timer = gate
            .observe_disconnected(now, disconnect_event("deadline"))
            .unwrap();

        assert_eq!(
            gate.observe_connected(timer.deadline),
            ConnectedEventAction::PublishDisconnectThenConnected(disconnect_event("deadline"))
        );
        assert!(gate.confirm_disconnect(&timer, timer.deadline).is_none());
    }

    #[test]
    fn early_timer_wakeup_does_not_confirm_the_disconnect() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();
        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );
        let timer = gate
            .observe_disconnected(now, disconnect_event("early"))
            .unwrap();

        assert!(
            gate.confirm_disconnect(&timer, timer.deadline - Duration::from_millis(1))
                .is_none()
        );
        assert_eq!(
            gate.confirm_disconnect(&timer, timer.deadline),
            Some(disconnect_event("early"))
        );
    }

    #[test]
    fn stale_confirmation_cannot_confirm_a_later_disconnect() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();
        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );

        let first_timer = gate
            .observe_disconnected(now, disconnect_event("first"))
            .unwrap();
        assert_eq!(
            gate.observe_connected(now + Duration::from_secs(1)),
            ConnectedEventAction::Ignore
        );
        let second_timer = gate
            .observe_disconnected(now + Duration::from_secs(2), disconnect_event("second"))
            .unwrap();

        assert!(
            gate.confirm_disconnect(&first_timer, second_timer.deadline)
                .is_none()
        );
        assert_eq!(
            gate.confirm_disconnect(&second_timer, second_timer.deadline),
            Some(disconnect_event("second"))
        );
    }

    #[test]
    fn expected_stop_cancels_a_pending_disconnect() {
        let mut gate = TunnelConnectivityGate::default();
        let now = Instant::now();
        assert_eq!(
            gate.observe_connected(now),
            ConnectedEventAction::PublishConnected
        );
        let timer = gate
            .observe_disconnected(now, disconnect_event("stop"))
            .unwrap();

        gate.set_expected_stop(true);

        assert!(gate.confirm_disconnect(&timer, timer.deadline).is_none());
        assert_eq!(
            gate.observe_connected(timer.deadline),
            ConnectedEventAction::Ignore
        );
        gate.set_expected_stop(false);
        assert_eq!(
            gate.observe_connected(timer.deadline),
            ConnectedEventAction::PublishConnected
        );
    }
}
