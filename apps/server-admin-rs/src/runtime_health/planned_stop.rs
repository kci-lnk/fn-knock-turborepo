//! Instance-bound, acknowledged stop intent. This channel deliberately does not use SQLite.
use super::*;

const MAX_STOP_SECONDS: u64 = 3600;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub(crate) struct Lifecycle {
    pub phase: String,
    pub operation_id: String,
    pub reason: String,
    pub requested_at: String,
    pub deadline_at: String,
    pub management_instance: String,
    pub gateway_instance: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    operation_id: String,
    management_instance: String,
    gateway_instance: String,
    action: String,
    requested_at: i64,
    timeout_seconds: u64,
}

struct ActiveStop {
    request: Request,
    view: Lifecycle,
    deadline: Instant,
}

#[derive(Default)]
pub(super) struct StopState {
    generation: u64,
    active: Option<ActiveStop>,
    // Requests have a short admission window. Remember accepted operations so cancellation
    // or expiry cannot be undone by a delayed writer replaying an older request.
    seen_operations: VecDeque<String>,
}

impl StopState {
    pub(super) fn view(&self) -> Option<Lifecycle> {
        self.active.as_ref().map(|active| active.view.clone())
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn suppresses(&self, id: &str, sampled_generation: Option<u64>) -> bool {
        affected(id)
            && (self.active.is_some()
                || sampled_generation.is_some_and(|generation| generation != self.generation))
    }

    fn accept(
        &mut self,
        request: Request,
        management: &str,
        gateway: &str,
        now: Instant,
        wall: i64,
    ) -> bool {
        if request.version != 1
            || request.management_instance != management
            || request.gateway_instance != gateway
            || !valid_token(&request.operation_id)
            || !(1..=MAX_STOP_SECONDS).contains(&request.timeout_seconds)
        {
            return false;
        }
        if let Some(active) = &mut self.active {
            if active.request.operation_id != request.operation_id {
                return false;
            }
            match request.action.as_str() {
                "prepare" => return now < active.deadline, // idempotent; never renew the lease
                "gateway_stopped" => {
                    active.view.phase = "stopped".into();
                    return false;
                }
                "cancel" => {
                    self.active = None;
                    self.generation += 1;
                    return false;
                }
                _ => return false,
            }
        }
        if request.action != "prepare"
            || self.seen_operations.contains(&request.operation_id)
            || request.requested_at > wall + 2
            || wall.saturating_sub(request.requested_at) > 5
        {
            return false;
        }
        self.generation += 1;
        self.seen_operations.push_back(request.operation_id.clone());
        // Admission is polled at most 10 times/s and requests age out after 5s.
        // 64 tombstones therefore cover all requests that could still be replayed.
        if self.seen_operations.len() > 64 {
            self.seen_operations.pop_front();
        }
        let view = Lifecycle {
            phase: "stopping".into(),
            operation_id: request.operation_id.clone(),
            reason: "platform_stop".into(),
            requested_at: time_utils::iso_from_ms(request.requested_at * 1000),
            deadline_at: time_utils::iso_from_ms((wall + request.timeout_seconds as i64) * 1000),
            management_instance: management.into(),
            gateway_instance: gateway.into(),
        };
        self.active = Some(ActiveStop {
            deadline: now + Duration::from_secs(request.timeout_seconds),
            request,
            view,
        });
        true
    }

    pub(super) fn replace_gateway(&mut self, instance: &str) -> Option<Lifecycle> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.request.gateway_instance != instance)
        {
            self.generation += 1;
            return self.active.take().map(|active| active.view);
        }
        None
    }

    fn expire(&mut self, now: Instant) -> Option<Lifecycle> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| now >= active.deadline)
        {
            let expired = self.active.take().map(|active| active.view);
            self.generation += 1;
            return expired;
        }
        None
    }
}

pub(super) fn affected(id: &str) -> bool {
    matches!(
        id,
        "gateway_process" | "gateway_dataplane" | "auth_bridge" | "config_sync"
    )
}

fn valid_token(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

pub(super) struct StopController {
    pub directory: PathBuf,
    state: StdMutex<StopState>,
    pub wake_probe: Notify,
    expected_start: StdMutex<Option<String>>,
    expected_instance: StdMutex<Option<String>>,
}

impl StopController {
    pub fn new(data_dir: &Path, supervisor: &str) -> Self {
        Self {
            directory: data_dir.join("runtime/planned-stop"),
            state: StdMutex::new(StopState::default()),
            wake_probe: Notify::new(),
            expected_instance: StdMutex::new(None),
            expected_start: StdMutex::new(if supervisor == "fpk" {
                std::env::var("FN_KNOCK_EXPECTED_GATEWAY_START").ok()
            } else {
                None
            }),
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, StopState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub async fn take_expected_start(&self, pid: Option<u32>, instance: &str) -> bool {
        if self
            .expected_instance
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_deref()
            == Some(instance)
        {
            return true;
        }
        let expected = self
            .expected_start
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        let (Some(expected), Some(pid)) = (expected, pid) else {
            return false;
        };
        let Ok(stat) = tokio::fs::read_to_string(format!("/proc/{pid}/stat")).await else {
            return false;
        };
        let matches = matches_expected_start(&expected, pid, &stat);
        if matches {
            *self
                .expected_instance
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(instance.to_string());
        }
        matches
    }

    pub fn view(&self) -> Option<Lifecycle> {
        self.lock()
            .active
            .as_ref()
            .map(|active| active.view.clone())
    }
}

async fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    tokio::fs::write(&temporary, bytes).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).await?;
    }
    tokio::fs::rename(temporary, path).await
}

fn matches_expected_start(expected: &str, pid: u32, stat: &str) -> bool {
    process_start_ticks(stat).is_some_and(|ticks| expected == format!("{pid}:{ticks}"))
}

// stat comm may contain spaces and parentheses; fields after the final ')' start at field 3.
fn process_start_ticks(raw: &str) -> Option<&str> {
    let value = raw.rsplit_once(')')?.1.split_whitespace().nth(19)?;
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then_some(value)
}

impl RuntimeHealth {
    pub(super) async fn start_planned_stop_control(&self, state: &AppState) -> anyhow::Result<()> {
        // FPK is the first protocol producer. Never interpret unbound legacy stop hints.
        if state.settings.runtime_target != "fpk" || !cfg!(target_os = "linux") {
            return Ok(());
        }
        let directory = &self.inner.planned_stop.directory;
        tokio::fs::create_dir_all(directory).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).await?;
        }
        let boot_id = tokio::fs::read_to_string("/proc/sys/kernel/random/boot_id").await?;
        let stat = tokio::fs::read_to_string(format!("/proc/{}/stat", std::process::id())).await?;
        let ticks = process_start_ticks(&stat)
            .ok_or_else(|| anyhow::anyhow!("missing management process identity"))?;
        atomic_write(
            &directory.join("manager"),
            format!(
                "{} {} {} {}\n",
                std::process::id(),
                ticks,
                self.inner.management_instance_id,
                boot_id.trim()
            )
            .as_bytes(),
        )
        .await?;
        let runtime = self.clone();
        let control_state = state.clone();
        state.spawn_background("runtime-planned-stop", async move {
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut descriptor = String::new();
            let mut last_error = Instant::now() - Duration::from_secs(30);
            loop {
                tokio::select! {
                    _ = control_state.shutdown.cancelled() => break,
                    _ = tick.tick() => {
                        if let Err(error) = runtime.poll_planned_stop(&mut descriptor).await
                            && last_error.elapsed() >= Duration::from_secs(30) {
                            tracing::warn!(%error, "planned stop control unavailable");
                            last_error = Instant::now();
                        }
                    }
                }
            }
        });
        Ok(())
    }

    async fn poll_planned_stop(&self, previous_descriptor: &mut String) -> anyhow::Result<()> {
        let controller = &self.inner.planned_stop;
        let expired = controller.lock().expire(Instant::now());
        if let Some(expired) = expired {
            controller.wake_probe.notify_one();
            self.inner.logger.log(
                "ERROR",
                "gateway_process",
                "stop_timeout",
                "planned_stop_expired",
                serde_json::from_value(json!({"operation_id": expired.operation_id}))
                    .unwrap_or_default(),
            );
            // Import through the same durable path as platform failures. Publishing to
            // SQLite here would block future ACKs when storage is busy.
            let hint = json!({"component":"gateway_process", "event":"stop_failed", "reason_code":"planned_stop_expired", "fields":{"operation_id":expired.operation_id}});
            tokio::fs::create_dir_all(&self.inner.supervisor_events_dir).await?;
            atomic_write(
                &self
                    .inner
                    .supervisor_events_dir
                    .join(format!("{}-stop_failed.json", expired.operation_id)),
                &serde_json::to_vec(&hint)?,
            )
            .await?;
        }
        let (gateway_pid, gateway_instance) = {
            let trackers = self.inner.trackers.lock().await;
            let Some(gateway) = trackers.get("gateway_process") else {
                return Ok(());
            };
            let (Some(pid), Some(instance)) =
                (gateway.health.pid, gateway.health.instance_id.clone())
            else {
                return Ok(());
            };
            (pid, instance)
        };
        if !valid_token(&gateway_instance) {
            return Ok(());
        }
        let management_instance = &self.inner.management_instance_id;
        let management_pid = std::process::id();
        // Do not rewrite the descriptor while the gateway is draining or gone.
        if controller.view().is_none()
            && previous_descriptor.split_whitespace().nth(5) != Some(gateway_instance.as_str())
            && let (Ok(management_stat), Ok(gateway_stat)) = (
                tokio::fs::read_to_string(format!("/proc/{management_pid}/stat")).await,
                tokio::fs::read_to_string(format!("/proc/{gateway_pid}/stat")).await,
            )
            && let (Some(management_ticks), Some(gateway_ticks)) = (
                process_start_ticks(&management_stat),
                process_start_ticks(&gateway_stat),
            )
        {
            let boot_id = tokio::fs::read_to_string("/proc/sys/kernel/random/boot_id").await?;
            let descriptor = format!(
                "1 {management_pid} {management_instance} {management_ticks} {gateway_pid} {gateway_instance} {gateway_ticks} {}\n",
                boot_id.trim()
            );
            if descriptor != *previous_descriptor {
                atomic_write(
                    &controller.directory.join("instance"),
                    descriptor.as_bytes(),
                )
                .await?;
                *previous_descriptor = descriptor;
            }
        }
        let request_path = controller.directory.join("request.json");
        let Ok(metadata) = tokio::fs::metadata(&request_path).await else {
            return Ok(());
        };
        if metadata.len() > 4096 {
            return Ok(());
        }
        let Ok(request) = serde_json::from_slice::<Request>(&tokio::fs::read(&request_path).await?)
        else {
            return Ok(());
        };
        let operation = request.operation_id.clone();
        let before = controller.view();
        let ack = controller.lock().accept(
            request,
            management_instance,
            &gateway_instance,
            Instant::now(),
            time_utils::now_ms() / 1000,
        );
        let after = controller.view();
        if before
            .as_ref()
            .map(|view| (&view.operation_id, &view.phase))
            != after.as_ref().map(|view| (&view.operation_id, &view.phase))
        {
            controller.wake_probe.notify_one();
            self.inner.logger.log(
                "INFO",
                "gateway_process",
                "planned_stop",
                if after.is_some() {
                    "platform_stop"
                } else {
                    "stop_canceled"
                },
                Map::from_iter([("operation_id".into(), json!(operation))]),
            );
        }
        if ack {
            // The state is committed before ACK. Health tracker commits use this same lock.
            let content = format!("{operation} {management_instance} {gateway_instance}\n");
            let path = controller.directory.join("ack");
            if tokio::fs::read(&path).await.ok().as_deref() != Some(content.as_bytes()) {
                atomic_write(&path, content.as_bytes()).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(action: &str) -> Request {
        Request {
            version: 1,
            operation_id: "op-1".into(),
            management_instance: "management-1".into(),
            gateway_instance: "gateway-1".into(),
            action: action.into(),
            requested_at: 100,
            timeout_seconds: 30,
        }
    }
    #[test]
    fn stop_is_scoped_and_never_suppresses_storage() {
        let mut state = StopState::default();
        let before = state.generation();
        assert!(state.accept(
            request("prepare"),
            "management-1",
            "gateway-1",
            Instant::now(),
            100
        ));
        for id in [
            "gateway_process",
            "gateway_dataplane",
            "auth_bridge",
            "config_sync",
        ] {
            assert!(state.suppresses(id, Some(before)));
        }
        assert!(!state.suppresses("storage", Some(before)));
        assert!(!state.suppresses("management", Some(before)));
    }
    #[test]
    fn expiry_cannot_be_renewed_or_replayed() {
        let mut state = StopState::default();
        let start = Instant::now();
        assert!(state.accept(request("prepare"), "management-1", "gateway-1", start, 100));
        assert!(state.accept(
            request("prepare"),
            "management-1",
            "gateway-1",
            start + Duration::from_secs(20),
            120
        ));
        assert!(state.expire(start + Duration::from_secs(30)).is_some());
        assert!(state.expire(start + Duration::from_secs(31)).is_none());
        assert!(!state.accept(request("prepare"), "management-1", "gateway-1", start, 100));
    }
    #[test]
    fn cancel_invalidates_inflight_results_but_not_new_probes() {
        let mut state = StopState::default();
        let sampled = state.generation();
        state.accept(
            request("prepare"),
            "management-1",
            "gateway-1",
            Instant::now(),
            100,
        );
        state.accept(
            request("cancel"),
            "management-1",
            "gateway-1",
            Instant::now(),
            101,
        );
        assert!(state.suppresses("gateway_process", Some(sampled)));
        assert!(!state.suppresses("gateway_process", Some(state.generation())));
        assert!(!state.accept(
            request("prepare"),
            "management-1",
            "gateway-1",
            Instant::now(),
            101
        ));
    }
    #[test]
    fn stale_or_wrong_instance_requests_never_mute_a_new_process() {
        for (management, gateway, wall) in [
            ("new-management", "gateway-1", 100),
            ("management-1", "new-gateway", 100),
            ("management-1", "gateway-1", 106),
            ("management-1", "gateway-1", 90),
        ] {
            let mut state = StopState::default();
            assert!(!state.accept(
                request("prepare"),
                management,
                gateway,
                Instant::now(),
                wall
            ));
            assert!(!state.suppresses("gateway_process", None));
        }
    }
    #[test]
    fn proc_identity_handles_spaces_and_parentheses() {
        assert_eq!(
            process_start_ticks(
                "42 (worker (one)) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987 0"
            ),
            Some("987")
        );
    }
    async fn setup_control() -> (tempfile::TempDir, AppState, Request) {
        let (directory, state) = crate::runtime_health::tests::runtime_test_state().await;
        let runtime = &state.runtime_health;
        tokio::fs::create_dir_all(&runtime.inner.planned_stop.directory)
            .await
            .unwrap();
        {
            let mut trackers = runtime.inner.trackers.lock().await;
            let tracker = trackers.get_mut("gateway_process").unwrap();
            tracker.health.pid = Some(std::process::id());
            tracker.health.instance_id = Some("gateway-1".into());
            tracker.health.status = HealthStatus::Healthy;
            // A healthy live gateway has already completed its first successful probe.
            tracker.health.last_success_at = Some(time_utils::now_iso());
        }
        runtime.publish_snapshot(&time_utils::now_iso()).await;
        let request = Request {
            management_instance: runtime.inner.management_instance_id.clone(),
            requested_at: time_utils::now_ms() / 1000,
            ..request("prepare")
        };
        (directory, state, request)
    }

    async fn submit(runtime: &RuntimeHealth, request: &Request) {
        let data = json!({"version": request.version, "operation_id":request.operation_id,
            "management_instance":request.management_instance, "gateway_instance":request.gateway_instance,
            "action":request.action, "requested_at":request.requested_at, "timeout_seconds":request.timeout_seconds});
        atomic_write(
            &runtime.inner.planned_stop.directory.join("request.json"),
            &serde_json::to_vec(&data).unwrap(),
        )
        .await
        .unwrap();
        runtime.poll_planned_stop(&mut String::new()).await.unwrap();
    }

    fn failed_probe(reason: &'static str) -> ProbeResult {
        ProbeResult {
            ok: false,
            reason_code: reason,
            metadata: ProbeMetadata::default(),
        }
    }

    #[tokio::test]
    async fn acknowledged_stop_blocks_inflight_gateway_failures_but_keeps_storage_and_normal_failures()
     {
        let (_directory, state, mut request) = setup_control().await;
        let runtime = &state.runtime_health;
        let generation = runtime.inner.planned_stop.lock().generation();
        assert!(runtime.component_ready("gateway_process").await);
        submit(runtime, &request).await;
        assert_eq!(
            tokio::fs::read_to_string(runtime.inner.planned_stop.directory.join("ack"))
                .await
                .unwrap(),
            format!(
                "{} {} {}\n",
                request.operation_id, request.management_instance, request.gateway_instance
            )
        );
        assert!(!runtime.component_ready("gateway_process").await);
        assert_eq!(
            runtime.snapshot().await.components["gateway_process"]
                .lifecycle
                .as_ref()
                .unwrap()
                .phase,
            "stopping"
        );
        // Replay the three 5-second probes from the incident, including a probe started before ACK.
        for _ in 0..3 {
            for id in [
                "gateway_process",
                "gateway_dataplane",
                "auth_bridge",
                "config_sync",
            ] {
                runtime
                    .apply_sampled_probe(
                        &state,
                        id,
                        failed_probe("service_unresponsive"),
                        &time_utils::now_iso(),
                        false,
                        Some(generation),
                    )
                    .await;
            }
            runtime
                .apply_sampled_blocked("auth_bridge", &time_utils::now_iso(), Some(generation))
                .await;
            runtime
                .apply_probe(
                    &state,
                    "storage",
                    failed_probe("sqlite_ping_failed"),
                    &time_utils::now_iso(),
                )
                .await;
        }
        let events = state
            .storage
            .store
            .list_system_events(1, 20, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events["total"], 1);
        assert_eq!(events["events"][0]["payload"]["component"], "storage");
        assert_eq!(
            runtime.inner.trackers.lock().await["gateway_process"]
                .health
                .consecutive_failures,
            0
        );

        request.action = "cancel".into();
        submit(runtime, &request).await;
        let fresh = runtime.inner.planned_stop.lock().generation();
        assert!(runtime.snapshot().await.lifecycle.is_none());
        // A result spanning cancellation is stale, then normal probes regain their original threshold.
        runtime
            .apply_sampled_probe(
                &state,
                "gateway_process",
                failed_probe("service_unresponsive"),
                &time_utils::now_iso(),
                false,
                Some(generation),
            )
            .await;
        for _ in 0..3 {
            runtime
                .apply_sampled_probe(
                    &state,
                    "gateway_process",
                    failed_probe("service_unresponsive"),
                    &time_utils::now_iso(),
                    false,
                    Some(fresh),
                )
                .await;
        }
        let events = state
            .storage
            .store
            .list_system_events(1, 20, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events["total"], 2);
        assert_eq!(
            runtime.inner.trackers.lock().await["gateway_process"]
                .health
                .status,
            HealthStatus::Unhealthy
        );
    }

    #[tokio::test]
    async fn stopping_preserves_existing_incident_and_does_not_suppress_abnormal_exit_or_stop_failure()
     {
        let (_directory, state, request) = setup_control().await;
        let runtime = &state.runtime_health;
        for _ in 0..3 {
            runtime
                .apply_probe(
                    &state,
                    "gateway_process",
                    failed_probe("service_unresponsive"),
                    &time_utils::now_iso(),
                )
                .await;
        }
        let incident = runtime.inner.trackers.lock().await["gateway_process"]
            .incident
            .as_ref()
            .unwrap()
            .id
            .clone();
        submit(runtime, &request).await;
        for _ in 0..2 {
            runtime
                .apply_probe(
                    &state,
                    "gateway_process",
                    ProbeResult {
                        ok: true,
                        reason_code: "serving",
                        metadata: ProbeMetadata::default(),
                    },
                    &time_utils::now_iso(),
                )
                .await;
        }
        assert_eq!(
            runtime.inner.trackers.lock().await["gateway_process"]
                .incident
                .as_ref()
                .unwrap()
                .id,
            incident
        );
        tokio::fs::create_dir_all(&runtime.inner.supervisor_events_dir)
            .await
            .unwrap();
        for (event, reason) in [
            ("exited", "oom_killed"),
            ("stop_failed", "forced_termination"),
        ] {
            atomic_write(
                &runtime
                    .inner
                    .supervisor_events_dir
                    .join(format!("{event}.json")),
                &serde_json::to_vec(
                    &json!({"component":"gateway_process", "event":event, "reason_code":reason}),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        }
        runtime.import_supervisor_hints(&state).await;
        let events = state
            .storage
            .store
            .list_system_events(1, 20, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        let types = events["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["type"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(types.contains(&"FN_EVENT_RUNTIME_HEALTH_FAILED"));
        assert!(types.contains(&"FN_EVENT_RUNTIME_STOP_FAILED"));
        assert!(types.contains(&"FN_EVENT_RUNTIME_ABNORMAL_EXIT"));
        assert!(!types.contains(&"FN_EVENT_RUNTIME_RECOVERED"));
    }

    #[test]
    fn replacement_or_wrong_operation_cannot_keep_new_gateway_muted() {
        let mut state = StopState::default();
        state.accept(
            request("prepare"),
            "management-1",
            "gateway-1",
            Instant::now(),
            100,
        );
        let mut other = request("cancel");
        other.operation_id = "other".into();
        state.accept(other, "management-1", "gateway-1", Instant::now(), 100);
        assert!(state.suppresses("gateway_process", None));
        assert!(state.replace_gateway("gateway-2").is_some());
        assert!(!state.suppresses("gateway_process", None));
    }
    #[test]
    fn expected_start_requires_exact_process_identity() {
        let stat = "42 (gateway) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987 0";
        assert!(matches_expected_start("42:987", 42, stat));
        assert!(!matches_expected_start("42:986", 42, stat));
        assert!(!matches_expected_start("43:987", 42, stat));
        assert!(!matches_expected_start("", 42, stat));
    }
    #[test]
    fn old_operation_cannot_replay_after_a_later_canceled_operation() {
        let mut state = StopState::default();
        let now = Instant::now();
        state.accept(request("prepare"), "management-1", "gateway-1", now, 100);
        state.accept(request("cancel"), "management-1", "gateway-1", now, 100);
        let mut next = request("prepare");
        next.operation_id = "op-2".into();
        assert!(state.accept(next.clone(), "management-1", "gateway-1", now, 100));
        next.action = "cancel".into();
        state.accept(next, "management-1", "gateway-1", now, 100);
        assert!(!state.accept(request("prepare"), "management-1", "gateway-1", now, 100));
    }

    #[tokio::test]
    async fn expired_controller_persists_one_failure_and_restores_readiness() {
        let (_directory, state, request) = setup_control().await;
        let runtime = &state.runtime_health;
        submit(runtime, &request).await;
        runtime
            .inner
            .planned_stop
            .lock()
            .active
            .as_mut()
            .unwrap()
            .deadline = Instant::now();
        runtime.poll_planned_stop(&mut String::new()).await.unwrap();
        runtime.poll_planned_stop(&mut String::new()).await.unwrap();
        assert!(!runtime.component_ready("gateway_process").await);
        assert_eq!(
            runtime.snapshot().await.components["gateway_process"].status,
            HealthStatus::Unknown
        );
        runtime
            .apply_probe(
                &state,
                "gateway_process",
                ProbeResult {
                    ok: true,
                    reason_code: "serving",
                    metadata: ProbeMetadata::default(),
                },
                &time_utils::now_iso(),
            )
            .await;
        runtime.publish_snapshot(&time_utils::now_iso()).await;
        assert!(runtime.component_ready("gateway_process").await);
        runtime.import_supervisor_hints(&state).await;
        runtime.import_supervisor_hints(&state).await;
        let events = state
            .storage
            .store
            .list_system_events(1, 20, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events["total"], 1);
        assert_eq!(events["events"][0]["type"], "FN_EVENT_RUNTIME_STOP_FAILED");
        assert_eq!(
            events["events"][0]["payload"]["operation_id"],
            request.operation_id
        );
    }
}
