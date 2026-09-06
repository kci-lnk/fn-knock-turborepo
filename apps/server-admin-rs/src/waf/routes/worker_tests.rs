use super::*;
use crate::grpc_proto::{
    self as pb,
    waf_service_server::{WafService, WafServiceServer},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering::SeqCst},
};
use std::time::Duration;
use tonic::{Request, Response, Status};

#[derive(Default)]
struct Gateway {
    pending: Mutex<Vec<pb::WafEvent>>,
    leased: Mutex<Vec<pb::WafEvent>>,
    changed: tokio::sync::Notify,
    stop: tokio_util::sync::CancellationToken,
    waits: AtomicUsize,
    active: AtomicUsize,
    drains: AtomicUsize,
    acks: AtomicUsize,
    releases: AtomicUsize,
    wait_error: AtomicUsize,
    ack_failures: AtomicUsize,
    drain_times: Mutex<Vec<tokio::time::Instant>>,
}

struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, SeqCst);
    }
}

impl Gateway {
    fn add(&self, count: usize) {
        let mut pending = self.pending.lock().unwrap();
        for _ in 0..count {
            pending.push(pb::WafEvent {
                trace_id: uuid::Uuid::new_v4().to_string(),
                time: crate::time_utils::now_iso(),
                action: "log".into(),
                rule_ids: vec![941100],
                ..Default::default()
            });
        }
        self.changed.notify_one();
    }
}

#[tonic::async_trait]
impl WafService for Arc<Gateway> {
    async fn get_waf_status(&self, _: Request<()>) -> Result<Response<pb::WafStatus>, Status> {
        Ok(Response::new(Default::default()))
    }
    async fn set_waf_config(
        &self,
        _: Request<pb::WafConfig>,
    ) -> Result<Response<pb::WafStatus>, Status> {
        Ok(Response::new(Default::default()))
    }
    async fn validate_waf_bundle(
        &self,
        _: Request<pb::WafBundleRequest>,
    ) -> Result<Response<pb::WafValidationResult>, Status> {
        Ok(Response::new(Default::default()))
    }
    async fn reload_waf_bundle(
        &self,
        _: Request<pb::WafBundleRequest>,
    ) -> Result<Response<pb::WafStatus>, Status> {
        Ok(Response::new(Default::default()))
    }
    async fn wait_waf_events(
        &self,
        req: Request<pb::WafWaitRequest>,
    ) -> Result<Response<pb::WafWaitResult>, Status> {
        assert_eq!(req.metadata().get("grpc-timeout").unwrap(), "65000000u");
        assert_eq!(req.get_ref().timeout_ms, 60_000);
        self.waits.fetch_add(1, SeqCst);
        self.active.fetch_add(1, SeqCst);
        let _active = Active(&self.active);
        match self.wait_error.load(SeqCst) {
            12 => return Err(Status::unimplemented("old gateway")),
            14 => return Err(Status::unavailable("restarting")),
            _ => {}
        }
        let timeout = tokio::time::sleep(Duration::from_secs(60));
        tokio::pin!(timeout);
        loop {
            let notified = self.changed.notified();
            if !self.pending.lock().unwrap().is_empty() {
                return Ok(Response::new(pb::WafWaitResult { available: true }));
            }
            tokio::select! {
                _ = self.stop.cancelled() => return Err(Status::unavailable("gateway shutdown")),
                _ = &mut timeout => return Ok(Response::new(pb::WafWaitResult { available: false })),
                _ = notified => {},
            }
        }
    }
    async fn drain_waf_events(
        &self,
        req: Request<pb::WafDrainRequest>,
    ) -> Result<Response<pb::WafDrainResult>, Status> {
        let mut pending = self.pending.lock().unwrap();
        let mut leased = self.leased.lock().unwrap();
        let mut result = pb::WafDrainResult::default();
        match pb::WafDrainOperation::try_from(req.get_ref().operation).unwrap() {
            pb::WafDrainOperation::Lease => {
                self.drains.fetch_add(1, SeqCst);
                self.drain_times
                    .lock()
                    .unwrap()
                    .push(tokio::time::Instant::now());
                let count = pending.len().min(req.get_ref().limit as usize);
                *leased = pending.drain(..count).collect();
                result.events = leased.clone();
                result.drained = count as i32;
                if count > 0 {
                    result.lease_id = "lease".into();
                }
                // Deliberately stale pre-ACK value: worker must use ACK remaining.
                result.remaining = 0;
            }
            pb::WafDrainOperation::Acknowledge => {
                self.acks.fetch_add(1, SeqCst);
                if self
                    .ack_failures
                    .fetch_update(SeqCst, SeqCst, |n| n.checked_sub(1))
                    .is_ok()
                {
                    pending.extend(leased.drain(..));
                    return Err(Status::unavailable("ACK lost; lease expired"));
                }
                result.acknowledged = leased.len() as i32;
                leased.clear();
                result.remaining = pending.len() as i32;
            }
            pb::WafDrainOperation::Release => {
                self.releases.fetch_add(1, SeqCst);
                pending.extend(leased.drain(..));
                result.remaining = pending.len() as i32;
            }
            _ => unreachable!(),
        }
        Ok(Response::new(result))
    }
}

fn serve(listener: tokio::net::TcpListener, service: Arc<Gateway>) -> tokio::task::JoinHandle<()> {
    let stop = service.stop.clone();
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(WafServiceServer::new(service))
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                stop.cancelled_owned(),
            )
            .await
            .unwrap();
    })
}

async fn fixture() -> (
    tempfile::TempDir,
    AppState,
    Arc<Gateway>,
    tokio::task::JoinHandle<()>,
) {
    let gateway = Arc::new(Gateway::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = serve(listener, gateway.clone());
    let (directory, state) = waf_test_state(&address.to_string()).await;
    set_settings(&state, true, 2).await;
    (directory, state, gateway, server)
}

async fn set_settings(state: &AppState, enabled: bool, interval: u64) {
    state
        .storage
        .store
        .set_config_top_level_value(
            "waf",
            json!({"enabled": enabled, "drain_interval_seconds": interval}),
        )
        .await
        .unwrap();
}

async fn until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

fn worker(state: &AppState) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move { super::super::drain_worker::run(&state).await })
}

#[tokio::test]
async fn long_wait_does_not_block_manual_drain_and_cancels_on_disable() {
    let (_dir, state, gateway, server) = fixture().await;
    let task = worker(&state);
    until(|| gateway.active.load(SeqCst) == 1).await;
    tokio::time::sleep(Duration::from_millis(150)).await; // exceeds ordinary 100ms RPC deadline
    assert_eq!(gateway.waits.load(SeqCst), 1);
    tokio::time::timeout(Duration::from_millis(300), drain_waf_events_now(&state))
        .await
        .unwrap()
        .unwrap();
    set_settings(&state, false, 2).await;
    until(|| gateway.active.load(SeqCst) == 0).await;
    let waits = gateway.waits.load(SeqCst);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(gateway.waits.load(SeqCst), waits);
    set_settings(&state, true, 2).await;
    until(|| gateway.active.load(SeqCst) == 1).await;
    state.shutdown.cancel();
    tokio::time::timeout(Duration::from_millis(300), task)
        .await
        .unwrap()
        .unwrap();
    until(|| gateway.active.load(SeqCst) == 0).await;
    server.abort();
}

#[tokio::test]
async fn batches_coalesce_without_config_starvation_and_clear_backlog_immediately() {
    let (_dir, state, gateway, server) = fixture().await;
    let task = worker(&state);
    until(|| gateway.active.load(SeqCst) == 1).await;
    let started = tokio::time::Instant::now();
    gateway.add(1000);
    for i in 0..8 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        state
            .storage
            .store
            .set_config_top_level_value("unrelated_test", json!(i))
            .await
            .unwrap();
    }
    assert_eq!(
        gateway.drains.load(SeqCst),
        1,
        "must aggregate before first batch"
    );
    until(|| gateway.acks.load(SeqCst) == 2).await;
    let times = gateway.drain_times.lock().unwrap().clone();
    assert_eq!(times.len(), 3);
    assert!(times[1].duration_since(started) >= Duration::from_millis(1900));
    assert!(times[1].duration_since(started) < Duration::from_millis(2800));
    assert!(
        times[2].duration_since(times[1]) < Duration::from_secs(1),
        "backlog waited an extra batch interval"
    );
    assert!(gateway.pending.lock().unwrap().is_empty());
    state.shutdown.cancel();
    task.await.unwrap();
    server.abort();
}

#[tokio::test]
async fn lost_ack_retries_idempotent_persistence() {
    let (_dir, state, gateway, server) = fixture().await;
    gateway.add(1);
    let id = gateway.pending.lock().unwrap()[0].trace_id.clone();
    gateway.ack_failures.store(1, SeqCst);
    let task = worker(&state);
    until(|| gateway.acks.load(SeqCst) == 2).await;
    assert!(
        state
            .storage
            .store
            .get_waf_log_event(&id)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(gateway.drains.load(SeqCst), 2);
    let db =
        tokio_rusqlite::rusqlite::Connection::open(_dir.path().join("fn-knock.sqlite3")).unwrap();
    let stored: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM kv_keys WHERE key LIKE 'fn_knock:waf:log:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, 1, "redelivery duplicated the durable event");
    state.shutdown.cancel();
    task.await.unwrap();
    server.abort();
}

#[tokio::test]
async fn persistence_failure_releases_lease_and_retries_after_recovery() {
    let (dir, state, gateway, server) = fixture().await;
    let db =
        tokio_rusqlite::rusqlite::Connection::open(dir.path().join("fn-knock.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_waf BEFORE INSERT ON kv_keys WHEN NEW.key LIKE 'fn_knock:waf:log:%' BEGIN SELECT RAISE(FAIL, 'injected disk write failure'); END;").unwrap();
    gateway.add(1);
    let id = gateway.pending.lock().unwrap()[0].trace_id.clone();
    let task = worker(&state);
    until(|| gateway.releases.load(SeqCst) == 1).await;
    assert_eq!(gateway.acks.load(SeqCst), 0);
    assert!(
        state
            .storage
            .store
            .get_waf_log_event(&id)
            .await
            .unwrap()
            .is_none()
    );
    db.execute_batch("DROP TRIGGER reject_waf;").unwrap();
    until(|| gateway.acks.load(SeqCst) == 1).await;
    assert!(
        state
            .storage
            .store
            .get_waf_log_event(&id)
            .await
            .unwrap()
            .is_some()
    );
    state.shutdown.cancel();
    task.await.unwrap();
    server.abort();
}

#[tokio::test]
async fn service_errors_back_off_and_disable_interrupts_backoff() {
    let (_dir, state, gateway, server) = fixture().await;
    gateway.wait_error.store(14, SeqCst);
    let task = worker(&state);
    until(|| gateway.waits.load(SeqCst) == 2).await;
    assert_eq!(
        gateway.drains.load(SeqCst),
        1,
        "service error must not enable legacy polling"
    );
    set_settings(&state, false, 2).await;
    let calls = gateway.waits.load(SeqCst);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(gateway.waits.load(SeqCst), calls);
    gateway.wait_error.store(0, SeqCst);
    set_settings(&state, true, 2).await;
    until(|| gateway.active.load(SeqCst) == 1).await;
    state.shutdown.cancel();
    task.await.unwrap();
    server.abort();
}

#[tokio::test]
async fn relevant_config_recomputes_batch_deadline() {
    let (_dir, state, gateway, server) = fixture().await;
    set_settings(&state, true, 60).await;
    let task = worker(&state);
    until(|| gateway.active.load(SeqCst) == 1).await;
    gateway.add(1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    set_settings(&state, true, 1).await;
    until(|| gateway.acks.load(SeqCst) == 1).await;
    state.shutdown.cancel();
    task.await.unwrap();
    server.abort();
}

#[tokio::test]
async fn gateway_restart_reconnects_and_collects_new_events() {
    let (_dir, state, gateway, server) = fixture().await;
    let task = worker(&state);
    until(|| gateway.active.load(SeqCst) == 1).await;
    gateway.stop.cancel();
    tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    until(|| gateway.active.load(SeqCst) == 0).await;
    let replacement = Arc::new(Gateway::default());
    let listener = tokio::net::TcpListener::bind(&state.settings.go_backend_grpc_addr)
        .await
        .unwrap();
    let server = serve(listener, replacement.clone());
    replacement.add(1);
    until(|| replacement.acks.load(SeqCst) == 1).await;
    state.shutdown.cancel();
    task.await.unwrap();
    server.abort();
}

#[tokio::test]
async fn legacy_gateway_is_reprobed_and_recovers_long_polling() {
    let (_dir, state, gateway, server) = fixture().await;
    gateway.wait_error.store(12, SeqCst);
    let task = worker(&state);
    until(|| gateway.waits.load(SeqCst) == 1).await;
    until(|| gateway.drains.load(SeqCst) >= 2).await;
    gateway.wait_error.store(0, SeqCst);
    // Real transport test: legacy polling continues until the 60s probe.
    tokio::time::timeout(Duration::from_secs(65), async {
        while gateway.waits.load(SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    until(|| gateway.active.load(SeqCst) == 1).await;
    let calls = gateway.drains.load(SeqCst);
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert_eq!(gateway.drains.load(SeqCst), calls);
    state.shutdown.cancel();
    task.await.unwrap();
    server.abort();
}

#[test]
fn retry_delays_cap_at_thirty_seconds() {
    let mut failures = 0;
    let actual: Vec<_> = (0..9)
        .map(|_| super::super::drain_worker::retry_delay(&mut failures).as_secs())
        .collect();
    assert_eq!(actual, [1, 2, 4, 8, 16, 30, 30, 30, 30]);
}

// Run against internal/wafwaitfixture from the Go repository. Separate test
// processes keep Rust and Go CPU/RSS measurements attributable to each mode.
#[tokio::test]
#[ignore = "manual real-time WAF interoperability A/B; requires loopback Go fixture"]
async fn waf_long_polling_ab() {
    let rpc = std::env::var("FN_KNOCK_WAF_FIXTURE_RPC").unwrap();
    let control = std::env::var("FN_KNOCK_WAF_FIXTURE_CONTROL").unwrap();
    let legacy = std::env::var("FN_KNOCK_WAF_AB_MODE").unwrap() == "old";
    let seconds: u64 = std::env::var("FN_KNOCK_WAF_AB_IDLE_SECONDS")
        .unwrap_or_else(|_| "600".into())
        .parse()
        .unwrap();
    let (_dir, state) = waf_test_state(&rpc).await;
    set_settings(&state, true, 2).await;
    let worker_state = state.clone();
    let task = if legacy {
        tokio::spawn(async move {
            let mut updates = worker_state.storage.store.subscribe_config_snapshot();
            drain_waf_events_now(&worker_state).await.unwrap();
            while wait_for_waf_drain(&worker_state, &mut updates).await {
                drain_waf_events_now(&worker_state).await.unwrap();
            }
        })
    } else {
        worker(&state)
    };
    let http = reqwest::Client::new();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let stats = async || -> Value {
        http.get(format!("{control}/stats"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    };
    let before = stats().await;
    println!(
        "WAF_AB {}",
        json!({"phase":"warm", "mode": if legacy {"old"} else {"new"}, "pid":std::process::id(), "go":before, "tasks":tokio::runtime::Handle::current().metrics().num_alive_tasks()})
    );
    let recorder = state.storage.store.diagnostics();
    let generation = recorder.start();
    let started = tokio::time::Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        tokio::time::sleep(
            Duration::from_secs(60)
                .min(Duration::from_secs(seconds).saturating_sub(started.elapsed())),
        )
        .await;
        println!(
            "WAF_AB {}",
            json!({"phase":"idle_sample", "elapsed_ms":started.elapsed().as_millis(), "go":stats().await, "tasks":tokio::runtime::Handle::current().metrics().num_alive_tasks()})
        );
    }
    recorder.stop(generation);
    let idle = recorder.snapshot();
    assert!(
        !idle
            .operations
            .iter()
            .any(|op| op.kind == "sqlite" && op.label.contains("config"))
    );
    let after = stats().await;
    if !legacy {
        assert_eq!(
            after["drains"], before["drains"],
            "idle long polling performed empty drains"
        );
    }
    println!(
        "WAF_AB {}",
        json!({"phase":"idle_done", "seconds":seconds, "go":after, "diagnostics":idle})
    );
    for count in [1, 1, 1, 1000] {
        let before = stats().await;
        let started = tokio::time::Instant::now();
        http.post(format!("{control}/events?count={count}"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let now = stats().await;
                if now["pending"] == 0 && now["acks"].as_i64().unwrap() > before["acks"].as_i64().unwrap() {
                    println!("WAF_AB {}", json!({"phase":"events", "count":count, "latency_ms":started.elapsed().as_millis(), "go":now}));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    let db =
        tokio_rusqlite::rusqlite::Connection::open(_dir.path().join("fn-knock.sqlite3")).unwrap();
    let stored: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM kv_keys WHERE key LIKE 'fn_knock:waf:log:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, 1003, "all synthetic events must be durable");
    state.shutdown.cancel();
    task.await.unwrap();
    println!(
        "WAF_AB {}",
        json!({"phase":"stopped", "go":stats().await, "tasks":tokio::runtime::Handle::current().metrics().num_alive_tasks()})
    );
}
