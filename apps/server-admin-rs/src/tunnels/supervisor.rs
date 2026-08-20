use std::{
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::{Mutex, mpsc, oneshot, watch},
    task::JoinHandle,
    time::{MissedTickBehavior, interval, sleep},
};
use tokio_util::sync::CancellationToken;

use crate::time_utils;

const BACKOFF_SECONDS: &[u64] = &[1, 2, 5, 10, 30, 60, 120, 300];
const STABLE_RUN_SECONDS: u64 = 5 * 60;
const EXTERNAL_PROCESS_POLL_SECONDS: u64 = 2;
const RESOURCE_SAMPLE_SECONDS: u64 = 30;
const LOG_LINE_MAX_CHARS: usize = 4096;
const OUTPUT_TAIL_LINES: usize = 20;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SupervisorPhase {
    #[default]
    Stopped,
    Starting,
    Running,
    Backoff,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResourceSample {
    pub sampled_at: Option<String>,
    pub resident_kib: Option<u64>,
    pub peak_resident_kib: Option<u64>,
    pub threads: Option<u64>,
    pub system_available_kib: Option<u64>,
    pub cgroup_oom_kill_count: Option<u64>,
    pub cgroup_memory_fail_count: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorFailure {
    pub at: String,
    pub started_at: Option<String>,
    pub reason: String,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub core_dumped: bool,
    pub uptime_ms: u64,
    pub diagnosis: Option<String>,
    pub resources: Option<ProcessResourceSample>,
    #[serde(skip)]
    pub recent_stdout: Vec<String>,
    #[serde(skip)]
    pub recent_stderr: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorSnapshot {
    pub state: SupervisorPhase,
    pub desired_running: bool,
    pub running: bool,
    pub attached: bool,
    pub pid: Option<u32>,
    pub restart_count: u64,
    pub consecutive_failures: u32,
    pub next_restart_at: Option<String>,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub last_failure: Option<SupervisorFailure>,
    pub last_message: Option<String>,
}

impl Default for SupervisorSnapshot {
    fn default() -> Self {
        Self {
            state: SupervisorPhase::Stopped,
            desired_running: false,
            running: false,
            attached: false,
            pid: None,
            restart_count: 0,
            consecutive_failures: 0,
            next_restart_at: None,
            started_at: None,
            stopped_at: None,
            last_failure: None,
            last_message: None,
        }
    }
}

impl SupervisorSnapshot {
    pub fn normalize(mut self) -> Self {
        if self.running && self.pid.is_some() {
            self.state = SupervisorPhase::Running;
            self.next_restart_at = None;
        } else {
            self.running = false;
            self.pid = None;
            self.attached = false;
            if self.state == SupervisorPhase::Running || self.state == SupervisorPhase::Starting {
                self.state = if self.desired_running {
                    SupervisorPhase::Backoff
                } else {
                    SupervisorPhase::Stopped
                };
            }
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub struct ProcessLaunch {
    pub executable: OsString,
    pub args: Vec<OsString>,
    pub current_dir: PathBuf,
}

#[async_trait]
pub trait TunnelProcessAdapter: Send + Sync + 'static {
    fn key(&self) -> String;
    fn label(&self) -> String;

    async fn prepare_launch(&self) -> Result<ProcessLaunch, String>;
    async fn find_existing_pid(&self) -> Option<u32>;
    async fn owns_live_pid(&self, pid: u32) -> bool;
    async fn terminate_process(&self, pid: u32) -> Result<(), String> {
        terminate_pid(pid).await
    }
    async fn persist_snapshot(&self, snapshot: &SupervisorSnapshot) -> Result<(), String>;
    fn sanitize_output(&self, line: &str) -> String {
        line.to_string()
    }
    async fn append_output(&self, stream: OutputStream, line: String);
    async fn append_supervisor_log(&self, line: String);
    async fn set_expected_stop(&self, expected: bool);
    async fn on_unexpected_exit(&self, pid: Option<u32>, failure: &SupervisorFailure);
    async fn remove_pid_file(&self);
    async fn write_pid_file(&self, pid: u32);
}

#[derive(Clone)]
pub struct SupervisorHandle {
    tx: mpsc::Sender<SupervisorCommand>,
    snapshot: watch::Receiver<SupervisorSnapshot>,
}

impl SupervisorHandle {
    pub fn snapshot(&self) -> SupervisorSnapshot {
        self.snapshot.borrow().clone()
    }

    pub async fn start(&self) -> Result<u32, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(SupervisorCommand::Start(reply_tx))
            .await
            .map_err(|_| "process supervisor is unavailable".to_string())?;
        reply_rx
            .await
            .map_err(|_| "process supervisor stopped before starting".to_string())?
    }

    pub async fn stop(&self) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(SupervisorCommand::Stop(reply_tx))
            .await
            .map_err(|_| "process supervisor is unavailable".to_string())?;
        reply_rx
            .await
            .map_err(|_| "process supervisor stopped before stopping".to_string())?
    }

    pub async fn restart(&self) -> Result<u32, String> {
        if !self.snapshot().desired_running {
            return self.start().await;
        }
        self.pause_for_restart().await?;
        self.start().await
    }

    pub async fn pause_for_restart(&self) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(SupervisorCommand::StopForRestart(reply_tx))
            .await
            .map_err(|_| "process supervisor is unavailable".to_string())?;
        reply_rx
            .await
            .map_err(|_| "process supervisor stopped before pausing".to_string())?
    }
}

enum SupervisorCommand {
    Start(oneshot::Sender<Result<u32, String>>),
    Stop(oneshot::Sender<Result<(), String>>),
    StopForRestart(oneshot::Sender<Result<(), String>>),
}

#[derive(Default)]
pub struct TunnelSupervisorRegistry {
    entries: Mutex<HashMap<String, SupervisorEntry>>,
}

struct SupervisorEntry {
    handle: SupervisorHandle,
    shutdown: CancellationToken,
    task: JoinHandle<()>,
}

impl TunnelSupervisorRegistry {
    pub async fn ensure(
        &self,
        adapter: Arc<dyn TunnelProcessAdapter>,
        initial: SupervisorSnapshot,
        shutdown: CancellationToken,
    ) -> SupervisorHandle {
        let key = adapter.key();
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.get(&key) {
            return entry.handle.clone();
        }
        let actor_shutdown = shutdown.child_token();
        let (handle, task) = spawn_supervisor_with_task(adapter, initial, actor_shutdown.clone());
        entries.insert(
            key,
            SupervisorEntry {
                handle: handle.clone(),
                shutdown: actor_shutdown,
                task,
            },
        );
        handle
    }

    pub async fn get(&self, key: &str) -> Option<SupervisorHandle> {
        self.entries
            .lock()
            .await
            .get(key)
            .map(|entry| entry.handle.clone())
    }

    pub async fn remove(&self, key: &str) -> Result<(), String> {
        let handle = self
            .entries
            .lock()
            .await
            .get(key)
            .map(|entry| entry.handle.clone());
        let Some(handle) = handle else {
            return Ok(());
        };
        handle.stop().await?;
        let entry = self.entries.lock().await.remove(key);
        if let Some(entry) = entry {
            entry.shutdown.cancel();
            await_actor_exit(entry.task, Duration::from_secs(5)).await;
        }
        Ok(())
    }

    pub async fn shutdown_all(&self, timeout: Duration) {
        let entries = {
            let mut entries = self.entries.lock().await;
            std::mem::take(&mut *entries)
                .into_values()
                .collect::<Vec<_>>()
        };
        for entry in &entries {
            entry.shutdown.cancel();
        }
        let started = Instant::now();
        for entry in entries {
            let remaining = timeout.saturating_sub(started.elapsed());
            await_actor_exit(entry.task, remaining).await;
        }
    }
}

#[cfg(test)]
fn spawn_supervisor(
    adapter: Arc<dyn TunnelProcessAdapter>,
    initial: SupervisorSnapshot,
    shutdown: CancellationToken,
) -> SupervisorHandle {
    spawn_supervisor_with_task(adapter, initial, shutdown).0
}

fn spawn_supervisor_with_task(
    adapter: Arc<dyn TunnelProcessAdapter>,
    initial: SupervisorSnapshot,
    shutdown: CancellationToken,
) -> (SupervisorHandle, JoinHandle<()>) {
    let initial = initial.normalize();
    let (tx, rx) = mpsc::channel(16);
    let (snapshot_tx, snapshot_rx) = watch::channel(initial.clone());
    let task = tokio::spawn(run_supervisor(adapter, initial, rx, snapshot_tx, shutdown));
    let handle = SupervisorHandle {
        tx,
        snapshot: snapshot_rx,
    };
    (handle, task)
}

async fn await_actor_exit(mut task: JoinHandle<()>, timeout: Duration) {
    if timeout.is_zero() || tokio::time::timeout(timeout, &mut task).await.is_err() {
        task.abort();
        let _ = task.await;
    }
}

struct SupervisorActor {
    adapter: Arc<dyn TunnelProcessAdapter>,
    snapshot: SupervisorSnapshot,
    commands: mpsc::Receiver<SupervisorCommand>,
    snapshots: watch::Sender<SupervisorSnapshot>,
    shutdown: CancellationToken,
    entropy: u64,
    retrying: bool,
}

async fn run_supervisor(
    adapter: Arc<dyn TunnelProcessAdapter>,
    initial: SupervisorSnapshot,
    commands: mpsc::Receiver<SupervisorCommand>,
    snapshots: watch::Sender<SupervisorSnapshot>,
    shutdown: CancellationToken,
) {
    let mut actor = SupervisorActor {
        adapter,
        snapshot: initial,
        commands,
        snapshots,
        shutdown,
        entropy: rand::random(),
        retrying: false,
    };
    actor.publish().await;
    if actor.snapshot.desired_running {
        if let Some(pid) = actor.adapter.find_existing_pid().await {
            actor.monitor_external(pid, None).await;
        } else {
            actor.retrying = true;
        }
    }
    actor.run().await;
}

impl SupervisorActor {
    async fn run(&mut self) {
        loop {
            if self.shutdown.is_cancelled() {
                return;
            }
            if self.snapshot.desired_running && self.retrying {
                if self.snapshot.state == SupervisorPhase::Backoff {
                    if !self.wait_backoff().await {
                        return;
                    }
                    continue;
                }
                if !self.snapshot.desired_running || !self.retrying {
                    continue;
                }
                match self.launch_process(None).await {
                    Ok(running) => {
                        self.retrying = false;
                        self.monitor_child(running, None).await;
                    }
                    Err(error) => {
                        self.record_spawn_failure(error).await;
                        if !self.wait_backoff().await {
                            return;
                        }
                    }
                }
                continue;
            }

            tokio::select! {
                _ = self.shutdown.cancelled() => return,
                command = self.commands.recv() => {
                    let Some(command) = command else { return; };
                    match command {
                        SupervisorCommand::Start(reply) => self.handle_start(reply).await,
                        SupervisorCommand::Stop(reply) => {
                            self.stop_without_process(reply).await;
                        }
                        SupervisorCommand::StopForRestart(reply) => {
                            self.stop_without_process_for_restart(reply).await;
                        }
                    }
                }
            }
        }
    }

    async fn handle_start(&mut self, reply: oneshot::Sender<Result<u32, String>>) {
        if self.snapshot.running
            && let Some(pid) = self.snapshot.pid
        {
            let result = self.set_desired_running_durable(true).await.map(|()| pid);
            let _ = reply.send(result);
            return;
        }
        let was_desired = self.snapshot.desired_running;
        let existing_pid = self.adapter.find_existing_pid().await;
        let prepared = if was_desired || existing_pid.is_some() {
            None
        } else {
            match self.adapter.prepare_launch().await {
                Ok(launch) => Some(launch),
                Err(error) => {
                    self.record_first_start_failure(&error).await;
                    let _ = reply.send(Err(error));
                    return;
                }
            }
        };
        if let Err(error) = self.set_desired_running_durable(true).await {
            let _ = reply.send(Err(error));
            return;
        }
        if let Some(pid) = existing_pid {
            self.monitor_external(pid, Some(reply)).await;
            return;
        }

        match self.launch_process(prepared).await {
            Ok(running) => {
                self.retrying = false;
                let pid = running.pid;
                let _ = reply.send(Ok(pid));
                self.monitor_child(running, None).await;
            }
            Err(error) => {
                if was_desired {
                    self.record_spawn_failure(error.clone()).await;
                    self.retrying = true;
                } else {
                    let previous = self.snapshot.clone();
                    self.record_first_start_failure(&error).await;
                    self.snapshot.desired_running = false;
                    if let Err(persist_error) = self.publish_required().await {
                        self.snapshot = previous;
                        self.broadcast();
                        self.record_spawn_failure(error.clone()).await;
                        self.retrying = true;
                        let _ = reply.send(Err(format!(
                            "{error}; failed to persist disabled retry state: {persist_error}"
                        )));
                        return;
                    }
                }
                let _ = reply.send(Err(error));
            }
        }
    }

    async fn stop_without_process(&mut self, reply: oneshot::Sender<Result<(), String>>) {
        let previous = self.snapshot.clone();
        self.snapshot.desired_running = false;
        self.snapshot.running = false;
        self.snapshot.attached = false;
        self.snapshot.pid = None;
        self.snapshot.state = SupervisorPhase::Stopped;
        self.snapshot.next_restart_at = None;
        self.snapshot.stopped_at = Some(time_utils::now_iso());
        self.snapshot.last_message = Some(format!("{} already stopped", self.adapter.label()));
        self.retrying = false;
        self.adapter.remove_pid_file().await;
        if let Err(error) = self.publish_required().await {
            self.snapshot = previous;
            self.broadcast();
            let _ = reply.send(Err(error));
        } else {
            let _ = reply.send(Ok(()));
        }
    }

    async fn stop_without_process_for_restart(
        &mut self,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        self.snapshot.desired_running = true;
        self.snapshot.running = false;
        self.snapshot.attached = false;
        self.snapshot.pid = None;
        self.snapshot.state = SupervisorPhase::Stopped;
        self.snapshot.next_restart_at = None;
        self.snapshot.stopped_at = Some(time_utils::now_iso());
        self.snapshot.last_message = Some(format!("{} restarting", self.adapter.label()));
        self.retrying = false;
        self.adapter.remove_pid_file().await;
        self.publish().await;
        let _ = reply.send(Ok(()));
    }

    async fn launch_process(
        &mut self,
        prepared: Option<ProcessLaunch>,
    ) -> Result<RunningChild, String> {
        self.snapshot.state = SupervisorPhase::Starting;
        self.snapshot.running = false;
        self.snapshot.attached = false;
        self.snapshot.pid = None;
        self.snapshot.next_restart_at = None;
        self.publish().await;

        let launch = match prepared {
            Some(launch) => launch,
            None => self.adapter.prepare_launch().await?,
        };
        let mut command = Command::new(&launch.executable);
        command
            .args(&launch.args)
            .current_dir(&launch.current_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        let mut child = command
            .spawn()
            .map_err(|error| format!("failed to spawn process: {error}"))?;
        let pid = child
            .id()
            .ok_or_else(|| "failed to read child process id".to_string())?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_reader = stdout
            .map(|reader| spawn_output_reader(self.adapter.clone(), OutputStream::Stdout, reader));
        let stderr_reader = stderr
            .map(|reader| spawn_output_reader(self.adapter.clone(), OutputStream::Stderr, reader));
        let waiter = tokio::spawn(async move { child.wait().await });
        let started = Instant::now();
        let resources_at_start = sample_process_resources(pid);

        self.snapshot.state = SupervisorPhase::Running;
        self.snapshot.running = true;
        self.snapshot.attached = true;
        self.snapshot.pid = Some(pid);
        self.snapshot.started_at = Some(time_utils::now_iso());
        self.snapshot.stopped_at = None;
        self.snapshot.next_restart_at = None;
        self.snapshot.last_message = Some(format!("{} started pid={pid}", self.adapter.label()));
        if self.retrying {
            self.snapshot.restart_count = self.snapshot.restart_count.saturating_add(1);
        }
        self.adapter.write_pid_file(pid).await;
        self.publish().await;
        self.adapter
            .append_supervisor_log(format!("{} started pid={pid}", self.adapter.label()))
            .await;

        Ok(RunningChild {
            pid,
            started,
            resources_at_start,
            last_resources: sample_process_resources(pid),
            waiter,
            stdout_reader,
            stderr_reader,
        })
    }

    async fn monitor_child(
        &mut self,
        mut running: RunningChild,
        start_reply: Option<oneshot::Sender<Result<u32, String>>>,
    ) {
        if let Some(reply) = start_reply {
            let _ = reply.send(Ok(running.pid));
        }
        let mut sampler = interval(Duration::from_secs(RESOURCE_SAMPLE_SECONDS));
        sampler.set_missed_tick_behavior(MissedTickBehavior::Delay);
        sampler.tick().await;
        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => {
                    if self.stop_running_child(&mut running, true, None).await {
                        return;
                    }
                }
                result = &mut running.waiter => {
                    let exit = match result {
                        Ok(result) => result,
                        Err(error) => Err(std::io::Error::other(error.to_string())),
                    };
                    self.finish_unexpected_child(running, exit).await;
                    return;
                }
                _ = sampler.tick() => {
                    running.last_resources = sample_process_resources(running.pid);
                    if running.started.elapsed() >= Duration::from_secs(STABLE_RUN_SECONDS)
                        && self.snapshot.consecutive_failures != 0
                    {
                        self.snapshot.consecutive_failures = 0;
                        self.publish().await;
                    }
                }
                command = self.commands.recv() => {
                    let Some(command) = command else {
                        self.stop_running_child(&mut running, true, None).await;
                        return;
                    };
                    match command {
                        SupervisorCommand::Start(reply) => {
                            let _ = reply.send(Ok(running.pid));
                        }
                        SupervisorCommand::Stop(reply) => {
                            if self.stop_running_child(&mut running, false, Some(reply)).await {
                                return;
                            }
                        }
                        SupervisorCommand::StopForRestart(reply) => {
                            if self.stop_running_child(&mut running, true, Some(reply)).await {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn monitor_external(
        &mut self,
        pid: u32,
        start_reply: Option<oneshot::Sender<Result<u32, String>>>,
    ) {
        self.snapshot.state = SupervisorPhase::Running;
        self.snapshot.desired_running = true;
        self.snapshot.running = true;
        self.snapshot.attached = false;
        self.snapshot.pid = Some(pid);
        self.snapshot.next_restart_at = None;
        if self.snapshot.started_at.is_none() {
            self.snapshot.started_at = Some(time_utils::now_iso());
        }
        self.snapshot.stopped_at = None;
        self.snapshot.last_message = Some(format!(
            "{} process adopted pid={pid}",
            self.adapter.label()
        ));
        self.adapter.write_pid_file(pid).await;
        self.publish().await;
        self.adapter
            .append_supervisor_log(format!(
                "{} process adopted pid={pid}",
                self.adapter.label()
            ))
            .await;
        if let Some(reply) = start_reply {
            let _ = reply.send(Ok(pid));
        }
        let started = Instant::now();
        let baseline = sample_process_resources(pid);
        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => {
                    if self.stop_external(pid, true, None).await {
                        return;
                    }
                }
                _ = sleep(Duration::from_secs(EXTERNAL_PROCESS_POLL_SECONDS)) => {
                    if !self.adapter.owns_live_pid(pid).await {
                        let failure = build_missing_process_failure(
                            started.elapsed(),
                            baseline.as_ref(),
                            sample_process_resources(pid),
                        );
                        self.finish_failure(Some(pid), failure).await;
                        return;
                    }
                    if started.elapsed() >= Duration::from_secs(STABLE_RUN_SECONDS)
                        && self.snapshot.consecutive_failures != 0
                    {
                        self.snapshot.consecutive_failures = 0;
                        self.publish().await;
                    }
                }
                command = self.commands.recv() => {
                    let Some(command) = command else {
                        self.stop_external(pid, true, None).await;
                        return;
                    };
                    match command {
                        SupervisorCommand::Start(reply) => {
                            let _ = reply.send(Ok(pid));
                        }
                        SupervisorCommand::Stop(reply) => {
                            if self.stop_external(pid, false, Some(reply)).await {
                                return;
                            }
                        }
                        SupervisorCommand::StopForRestart(reply) => {
                            if self.stop_external(pid, true, Some(reply)).await {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn stop_running_child(
        &mut self,
        running: &mut RunningChild,
        preserve_desired: bool,
        reply: Option<oneshot::Sender<Result<(), String>>>,
    ) -> bool {
        let previous = self.snapshot.clone();
        if !preserve_desired {
            self.snapshot.desired_running = false;
            if let Err(error) = self.publish_required().await {
                self.snapshot = previous;
                self.broadcast();
                if let Some(reply) = reply {
                    let _ = reply.send(Err(error));
                }
                return false;
            }
        }
        self.adapter.set_expected_stop(true).await;
        let mut result = self.adapter.terminate_process(running.pid).await;
        if result.is_err() && self.adapter.owns_live_pid(running.pid).await {
            self.adapter.set_expected_stop(false).await;
            self.snapshot.running = true;
            self.snapshot.attached = true;
            self.snapshot.pid = Some(running.pid);
            self.snapshot.state = SupervisorPhase::Running;
            self.snapshot.next_restart_at = None;
            self.snapshot.last_message = Some(format!(
                "{} termination failed for pid={}",
                self.adapter.label(),
                running.pid
            ));
            self.publish().await;
            if let Some(reply) = reply {
                let _ = reply.send(result);
            }
            sleep(Duration::from_millis(250)).await;
            return false;
        }
        if result.is_err() {
            result = Ok(());
        }
        let _ = tokio::time::timeout(Duration::from_secs(4), &mut running.waiter).await;
        drain_reader(running.stdout_reader.take()).await;
        drain_reader(running.stderr_reader.take()).await;
        self.adapter.set_expected_stop(false).await;
        self.adapter.remove_pid_file().await;
        self.snapshot.desired_running = preserve_desired;
        self.snapshot.running = false;
        self.snapshot.attached = false;
        self.snapshot.pid = None;
        self.snapshot.state = SupervisorPhase::Stopped;
        self.snapshot.next_restart_at = None;
        self.snapshot.stopped_at = Some(time_utils::now_iso());
        self.snapshot.last_message = Some(format!(
            "{} stopped pid={}",
            self.adapter.label(),
            running.pid
        ));
        self.publish().await;
        if !preserve_desired {
            self.adapter
                .append_supervisor_log(format!(
                    "{} stopped pid={}",
                    self.adapter.label(),
                    running.pid
                ))
                .await;
        }
        if let Some(reply) = reply {
            let _ = reply.send(result);
        }
        true
    }

    async fn stop_external(
        &mut self,
        pid: u32,
        preserve_desired: bool,
        reply: Option<oneshot::Sender<Result<(), String>>>,
    ) -> bool {
        let previous = self.snapshot.clone();
        if !preserve_desired {
            self.snapshot.desired_running = false;
            if let Err(error) = self.publish_required().await {
                self.snapshot = previous;
                self.broadcast();
                if let Some(reply) = reply {
                    let _ = reply.send(Err(error));
                }
                return false;
            }
        }
        self.adapter.set_expected_stop(true).await;
        let mut result = self.adapter.terminate_process(pid).await;
        if result.is_err() && self.adapter.owns_live_pid(pid).await {
            self.adapter.set_expected_stop(false).await;
            self.snapshot.running = true;
            self.snapshot.attached = false;
            self.snapshot.pid = Some(pid);
            self.snapshot.state = SupervisorPhase::Running;
            self.snapshot.next_restart_at = None;
            self.snapshot.last_message = Some(format!(
                "{} termination failed for pid={pid}",
                self.adapter.label()
            ));
            self.publish().await;
            if let Some(reply) = reply {
                let _ = reply.send(result);
            }
            sleep(Duration::from_millis(250)).await;
            return false;
        }
        if result.is_err() {
            result = Ok(());
        }
        self.adapter.set_expected_stop(false).await;
        self.adapter.remove_pid_file().await;
        self.snapshot.desired_running = preserve_desired;
        self.snapshot.running = false;
        self.snapshot.attached = false;
        self.snapshot.pid = None;
        self.snapshot.state = SupervisorPhase::Stopped;
        self.snapshot.next_restart_at = None;
        self.snapshot.stopped_at = Some(time_utils::now_iso());
        self.snapshot.last_message = Some(format!("{} stopped pid={pid}", self.adapter.label()));
        self.publish().await;
        if !preserve_desired {
            self.adapter
                .append_supervisor_log(format!("{} stopped pid={pid}", self.adapter.label()))
                .await;
        }
        if let Some(reply) = reply {
            let _ = reply.send(result);
        }
        true
    }

    async fn finish_unexpected_child(
        &mut self,
        mut running: RunningChild,
        exit: std::io::Result<ExitStatus>,
    ) {
        let stdout = drain_reader(running.stdout_reader.take()).await;
        let stderr = drain_reader(running.stderr_reader.take()).await;
        let resources = sample_process_resources(running.pid).or(running.last_resources);
        let failure = build_exit_failure(
            exit,
            running.started.elapsed(),
            running.resources_at_start.as_ref(),
            resources,
            stdout,
            stderr,
        );
        self.finish_failure(Some(running.pid), failure).await;
    }

    async fn finish_failure(&mut self, pid: Option<u32>, failure: SupervisorFailure) {
        self.adapter.remove_pid_file().await;
        self.snapshot.running = false;
        self.snapshot.attached = false;
        self.snapshot.pid = None;
        self.snapshot.stopped_at = Some(failure.at.clone());
        self.snapshot.last_message = Some(failure.reason.clone());
        self.snapshot.last_failure = Some(failure.clone());
        self.snapshot.consecutive_failures = next_consecutive_failures(
            self.snapshot.consecutive_failures,
            Duration::from_millis(failure.uptime_ms),
        );
        self.retrying = self.snapshot.desired_running;
        self.adapter.on_unexpected_exit(pid, &failure).await;
        if self.snapshot.desired_running {
            self.enter_backoff().await;
        } else {
            self.snapshot.state = SupervisorPhase::Stopped;
            self.snapshot.next_restart_at = None;
            self.publish().await;
        }
    }

    async fn record_spawn_failure(&mut self, error: String) {
        let failure = SupervisorFailure {
            at: time_utils::now_iso(),
            reason: format!("{} start failed: {error}", self.adapter.label()),
            ..SupervisorFailure::default()
        };
        self.snapshot.last_failure = Some(failure.clone());
        self.snapshot.last_message = Some(failure.reason.clone());
        self.snapshot.consecutive_failures = self.snapshot.consecutive_failures.saturating_add(1);
        self.adapter.on_unexpected_exit(None, &failure).await;
        self.enter_backoff().await;
    }

    async fn enter_backoff(&mut self) {
        let delay = backoff_delay(self.snapshot.consecutive_failures, self.next_entropy());
        self.snapshot.state = SupervisorPhase::Backoff;
        self.snapshot.running = false;
        self.snapshot.pid = None;
        self.snapshot.next_restart_at = Some(next_restart_at(delay, time_utils::now_ms()));
        self.snapshot.last_message = Some(format!(
            "{} will restart in {}s",
            self.adapter.label(),
            delay.as_secs()
        ));
        self.publish().await;
        self.adapter
            .append_supervisor_log(format!(
                "{} scheduled restart attempt={} in {}s",
                self.adapter.label(),
                self.snapshot.consecutive_failures,
                delay.as_secs()
            ))
            .await;
    }

    async fn wait_backoff(&mut self) -> bool {
        let delay = remaining_backoff(self.snapshot.next_restart_at.as_deref());
        tokio::select! {
            _ = self.shutdown.cancelled() => false,
            _ = sleep(delay) => {
                self.snapshot.next_restart_at = None;
                self.snapshot.state = SupervisorPhase::Starting;
                self.retrying = self.snapshot.desired_running;
                true
            }
            command = self.commands.recv() => {
                match command {
                    Some(SupervisorCommand::Start(reply)) => {
                        self.snapshot.next_restart_at = None;
                        self.retrying = true;
                        match self.launch_process(None).await {
                            Ok(running) => {
                                self.retrying = false;
                                let _ = reply.send(Ok(running.pid));
                                self.monitor_child(running, None).await;
                            }
                            Err(error) => {
                                self.record_spawn_failure(error.clone()).await;
                                self.retrying = true;
                                let _ = reply.send(Err(error));
                            }
                        }
                        true
                    }
                    Some(SupervisorCommand::Stop(reply)) => {
                        self.stop_without_process(reply).await;
                        true
                    }
                    Some(SupervisorCommand::StopForRestart(reply)) => {
                        self.stop_without_process_for_restart(reply).await;
                        true
                    }
                    None => false,
                }
            }
        }
    }

    fn next_entropy(&mut self) -> u64 {
        self.entropy = self
            .entropy
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.entropy
    }

    async fn publish(&mut self) {
        self.broadcast();
        if let Err(error) = self.adapter.persist_snapshot(&self.snapshot).await {
            self.log_persist_error(&error);
        }
    }

    async fn publish_required(&mut self) -> Result<(), String> {
        self.broadcast();
        self.adapter
            .persist_snapshot(&self.snapshot)
            .await
            .map_err(|error| {
                self.log_persist_error(&error);
                format!("failed to persist tunnel supervisor state: {error}")
            })
    }

    async fn set_desired_running_durable(&mut self, desired: bool) -> Result<(), String> {
        if self.snapshot.desired_running == desired {
            return Ok(());
        }
        let previous = self.snapshot.clone();
        self.snapshot.desired_running = desired;
        if let Err(error) = self.publish_required().await {
            self.snapshot = previous;
            self.broadcast();
            return Err(error);
        }
        Ok(())
    }

    async fn record_first_start_failure(&mut self, error: &str) {
        let failure = SupervisorFailure {
            at: time_utils::now_iso(),
            reason: format!("{} start failed: {error}", self.adapter.label()),
            ..SupervisorFailure::default()
        };
        self.snapshot.state = SupervisorPhase::Stopped;
        self.snapshot.last_message = Some(failure.reason.clone());
        self.snapshot.last_failure = Some(failure);
        self.publish().await;
        self.adapter
            .append_supervisor_log(format!("{} start failed: {error}", self.adapter.label()))
            .await;
    }

    fn broadcast(&self) {
        let _ = self.snapshots.send(self.snapshot.clone());
    }

    fn log_persist_error(&self, error: &str) {
        tracing::warn!(
            supervisor = %self.adapter.key(),
            %error,
            "failed to persist tunnel supervisor state"
        );
    }
}

struct RunningChild {
    pid: u32,
    started: Instant,
    resources_at_start: Option<ProcessResourceSample>,
    last_resources: Option<ProcessResourceSample>,
    waiter: JoinHandle<std::io::Result<ExitStatus>>,
    stdout_reader: Option<OutputCapture>,
    stderr_reader: Option<OutputCapture>,
}

struct OutputCapture {
    reader: JoinHandle<()>,
    writer: JoinHandle<()>,
    tail: Arc<StdMutex<OutputTail>>,
}

#[derive(Clone, Default)]
struct OutputTail {
    lines: Vec<String>,
    read_error: Option<String>,
}

fn spawn_output_reader<R>(
    adapter: Arc<dyn TunnelProcessAdapter>,
    stream: OutputStream,
    reader: R,
) -> OutputCapture
where
    R: AsyncRead + Unpin + Send + 'static,
{
    const PERSIST_QUEUE_CAPACITY: usize = 256;
    let tail = Arc::new(StdMutex::new(OutputTail::default()));
    let reader_tail = tail.clone();
    let (output_tx, mut output_rx) = mpsc::channel(PERSIST_QUEUE_CAPACITY);
    let writer_adapter = adapter.clone();
    let writer = tokio::spawn(async move {
        while let Some(line) = output_rx.recv().await {
            writer_adapter.append_output(stream, line).await;
        }
    });
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        let mut dropped_lines = 0_u64;
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let line = truncate_text(line.trim_end(), LOG_LINE_MAX_CHARS);
                    let line = adapter.sanitize_output(&line);
                    let line = bounded_log_line(&line);
                    if line.is_empty() {
                        continue;
                    }
                    {
                        let mut tail = reader_tail
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        push_tail(&mut tail.lines, line.clone());
                    }
                    match output_tx.try_send(line) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            dropped_lines = dropped_lines.saturating_add(1);
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            let mut tail = reader_tail
                                .lock()
                                .unwrap_or_else(|error| error.into_inner());
                            tail.read_error =
                                Some("log persistence worker stopped unexpectedly".to_string());
                            break;
                        }
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let mut tail = reader_tail
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    tail.read_error = Some(error.to_string());
                    break;
                }
            }
        }
        if dropped_lines != 0 {
            let mut tail = reader_tail
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            tail.read_error = Some(format!(
                "log persistence backlog overflowed; dropped {dropped_lines} lines"
            ));
        }
    });
    OutputCapture {
        reader,
        writer,
        tail,
    }
}

async fn drain_reader(capture: Option<OutputCapture>) -> OutputTail {
    let Some(mut capture) = capture else {
        return OutputTail::default();
    };
    if tokio::time::timeout(Duration::from_secs(5), &mut capture.reader)
        .await
        .is_err()
    {
        capture.reader.abort();
        let _ = capture.reader.await;
        let mut tail = capture
            .tail
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        tail.read_error = Some("log pipe did not close within 5 seconds".to_string());
    }
    if tokio::time::timeout(Duration::from_secs(2), &mut capture.writer)
        .await
        .is_err()
    {
        capture.writer.abort();
        let _ = capture.writer.await;
        let mut tail = capture
            .tail
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        tail.read_error = Some(match tail.read_error.take() {
            Some(error) => format!("{error}; log persistence did not drain within 2 seconds"),
            None => "log persistence did not drain within 2 seconds".to_string(),
        });
    }
    capture
        .tail
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn push_tail(lines: &mut Vec<String>, line: String) {
    if lines.len() >= OUTPUT_TAIL_LINES {
        lines.remove(0);
    }
    lines.push(line);
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut value = value.chars().take(max_chars).collect::<String>();
    value.push_str("...");
    value
}

pub fn bounded_log_line(value: &str) -> String {
    truncate_text(value, LOG_LINE_MAX_CHARS)
}

pub fn backoff_delay(consecutive_failures: u32, entropy: u64) -> Duration {
    let index = consecutive_failures.saturating_sub(1) as usize;
    let base = BACKOFF_SECONDS[index.min(BACKOFF_SECONDS.len() - 1)];
    let spread_ms = base.saturating_mul(1000) / 5;
    let width = spread_ms.saturating_mul(2).saturating_add(1);
    let offset = (entropy % width) as i64 - spread_ms as i64;
    let milliseconds = (base.saturating_mul(1000) as i64 + offset).max(1);
    Duration::from_millis(milliseconds as u64)
}

fn next_consecutive_failures(previous: u32, uptime: Duration) -> u32 {
    if uptime >= Duration::from_secs(STABLE_RUN_SECONDS) {
        1
    } else {
        previous.saturating_add(1)
    }
}

fn next_restart_at(delay: Duration, now_ms: i64) -> String {
    let delay_ms = delay.as_millis().min(i64::MAX as u128) as i64;
    time_utils::iso_from_ms(now_ms.saturating_add(delay_ms))
}

fn remaining_backoff(next_restart_at: Option<&str>) -> Duration {
    let Some(next) = next_restart_at.and_then(time_utils::parse_iso_ms) else {
        return Duration::ZERO;
    };
    Duration::from_millis(next.saturating_sub(time_utils::now_ms()).max(0) as u64)
}

fn build_exit_failure(
    exit: std::io::Result<ExitStatus>,
    uptime: Duration,
    baseline: Option<&ProcessResourceSample>,
    resources: Option<ProcessResourceSample>,
    stdout: OutputTail,
    stderr: OutputTail,
) -> SupervisorFailure {
    let exited_at_ms = time_utils::now_ms();
    let (exit_code, signal, core_dumped, reason) = match exit {
        Ok(status) => {
            let details = exit_status_details(&status);
            let reason = if let Some(signal) = details.1 {
                format!("process terminated by signal {signal}")
            } else {
                format!("process exited with code {}", details.0.unwrap_or(-1))
            };
            (details.0, details.1, details.2, reason)
        }
        Err(error) => (None, None, false, format!("process wait failed: {error}")),
    };
    let diagnosis = diagnose_exit(signal, baseline, resources.as_ref());
    let read_errors = [stdout.read_error.as_deref(), stderr.read_error.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let reason = if read_errors.is_empty() {
        reason
    } else {
        format!("{reason}; log read error: {}", read_errors.join("; "))
    };
    SupervisorFailure {
        at: time_utils::iso_from_ms(exited_at_ms),
        started_at: Some(time_utils::iso_from_ms(
            exited_at_ms.saturating_sub(uptime.as_millis().min(i64::MAX as u128) as i64),
        )),
        reason,
        exit_code,
        signal,
        core_dumped,
        uptime_ms: uptime.as_millis().min(u128::from(u64::MAX)) as u64,
        diagnosis,
        resources,
        recent_stdout: stdout.lines,
        recent_stderr: stderr.lines,
    }
}

fn build_missing_process_failure(
    uptime: Duration,
    baseline: Option<&ProcessResourceSample>,
    resources: Option<ProcessResourceSample>,
) -> SupervisorFailure {
    let exited_at_ms = time_utils::now_ms();
    SupervisorFailure {
        at: time_utils::iso_from_ms(exited_at_ms),
        started_at: Some(time_utils::iso_from_ms(
            exited_at_ms.saturating_sub(uptime.as_millis().min(i64::MAX as u128) as i64),
        )),
        reason: "adopted process is no longer running".to_string(),
        uptime_ms: uptime.as_millis().min(u128::from(u64::MAX)) as u64,
        diagnosis: diagnose_exit(None, baseline, resources.as_ref()),
        resources,
        ..SupervisorFailure::default()
    }
}

#[cfg(unix)]
fn exit_status_details(status: &ExitStatus) -> (Option<i32>, Option<i32>, bool) {
    use std::os::unix::process::ExitStatusExt;
    (status.code(), status.signal(), status.core_dumped())
}

#[cfg(not(unix))]
fn exit_status_details(status: &ExitStatus) -> (Option<i32>, Option<i32>, bool) {
    (status.code(), None, false)
}

fn diagnose_exit(
    signal: Option<i32>,
    baseline: Option<&ProcessResourceSample>,
    current: Option<&ProcessResourceSample>,
) -> Option<String> {
    #[cfg(unix)]
    if signal == Some(libc::SIGKILL)
        && counter_increased(
            baseline.and_then(|sample| sample.cgroup_oom_kill_count),
            current.and_then(|sample| sample.cgroup_oom_kill_count),
        )
    {
        return Some("suspected cgroup OOM kill".to_string());
    }
    None
}

fn counter_increased(before: Option<u64>, after: Option<u64>) -> bool {
    matches!((before, after), (Some(before), Some(after)) if after > before)
}

pub fn sample_process_resources(pid: u32) -> Option<ProcessResourceSample> {
    #[cfg(target_os = "linux")]
    {
        let process = std::fs::read_to_string(format!("/proc/{pid}/status")).ok();
        let memory = std::fs::read_to_string("/proc/meminfo").ok();
        let cgroup = read_cgroup_memory_counters();
        let sample = ProcessResourceSample {
            sampled_at: Some(time_utils::now_iso()),
            resident_kib: process
                .as_deref()
                .and_then(|value| parse_proc_kib(value, "VmRSS")),
            peak_resident_kib: process
                .as_deref()
                .and_then(|value| parse_proc_kib(value, "VmHWM")),
            threads: process
                .as_deref()
                .and_then(|value| parse_proc_number(value, "Threads")),
            system_available_kib: memory
                .as_deref()
                .and_then(|value| parse_proc_kib(value, "MemAvailable")),
            cgroup_oom_kill_count: cgroup.as_ref().and_then(|value| value.0),
            cgroup_memory_fail_count: cgroup.as_ref().and_then(|value| value.1),
        };
        let has_data = sample.resident_kib.is_some()
            || sample.peak_resident_kib.is_some()
            || sample.threads.is_some()
            || sample.system_available_kib.is_some()
            || sample.cgroup_oom_kill_count.is_some()
            || sample.cgroup_memory_fail_count.is_some();
        has_data.then_some(sample)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

#[cfg(any(target_os = "linux", test))]
fn parse_proc_number(content: &str, key: &str) -> Option<u64> {
    content.lines().find_map(|line| {
        let (candidate, value) = line.split_once(':')?;
        (candidate.trim() == key)
            .then(|| value.split_whitespace().next()?.parse::<u64>().ok())
            .flatten()
    })
}

#[cfg(any(target_os = "linux", test))]
fn parse_proc_kib(content: &str, key: &str) -> Option<u64> {
    parse_proc_number(content, key)
}

#[cfg(target_os = "linux")]
fn read_cgroup_memory_counters() -> Option<(Option<u64>, Option<u64>)> {
    let membership = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    if let Some(path) = membership.lines().find_map(|line| line.strip_prefix("0::")) {
        let relative = path.trim_start_matches('/');
        let events = std::path::Path::new("/sys/fs/cgroup")
            .join(relative)
            .join("memory.events");
        if let Ok(content) = std::fs::read_to_string(events) {
            return Some(parse_cgroup_v2_events(&content));
        }
    }
    let path = membership.lines().find_map(|line| {
        let mut parts = line.splitn(3, ':');
        let _ = parts.next()?;
        let controllers = parts.next()?;
        let path = parts.next()?;
        controllers
            .split(',')
            .any(|controller| controller == "memory")
            .then_some(path)
    })?;
    let memory_dir =
        std::path::Path::new("/sys/fs/cgroup/memory").join(path.trim_start_matches('/'));
    let oom_kills = std::fs::read_to_string(memory_dir.join("memory.oom_control"))
        .ok()
        .and_then(|value| parse_cgroup_v1_oom_control(&value));
    let failures = std::fs::read_to_string(memory_dir.join("memory.failcnt"))
        .ok()
        .and_then(|value| parse_cgroup_v1_failcnt(&value));
    (oom_kills.is_some() || failures.is_some()).then_some((oom_kills, failures))
}

#[cfg(any(target_os = "linux", test))]
fn parse_cgroup_v2_events(content: &str) -> (Option<u64>, Option<u64>) {
    (
        parse_space_counter(content, "oom_kill"),
        parse_space_counter(content, "max"),
    )
}

#[cfg(any(target_os = "linux", test))]
fn parse_cgroup_v1_failcnt(content: &str) -> Option<u64> {
    content.trim().parse::<u64>().ok()
}

#[cfg(any(target_os = "linux", test))]
fn parse_cgroup_v1_oom_control(content: &str) -> Option<u64> {
    parse_space_counter(content, "oom_kill")
}

#[cfg(any(target_os = "linux", test))]
fn parse_space_counter(content: &str, key: &str) -> Option<u64> {
    content.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        (parts.next()? == key)
            .then(|| parts.next()?.parse::<u64>().ok())
            .flatten()
    })
}

async fn terminate_pid(pid: u32) -> Result<(), String> {
    if pid == std::process::id() {
        return Err("refusing to terminate supervisor process".to_string());
    }
    #[cfg(unix)]
    {
        let pid_i32 = i32::try_from(pid).map_err(|_| "invalid process id".to_string())?;
        if !crate::unix::process_exists(pid_i32) {
            return Ok(());
        }
        crate::unix::send_signal(pid_i32, libc::SIGTERM)
            .map_err(|error| format!("failed to send SIGTERM: {error}"))?;
        for _ in 0..20 {
            if !crate::unix::process_exists(pid_i32) {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        crate::unix::send_signal(pid_i32, libc::SIGKILL)
            .map_err(|error| format!("failed to send SIGKILL: {error}"))?;
        for _ in 0..10 {
            if !crate::unix::process_exists(pid_i32) {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        (!crate::unix::process_exists(pid_i32))
            .then_some(())
            .ok_or_else(|| format!("process is still running: {pid}"))
    }
    #[cfg(windows)]
    {
        let taskkill = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|root| root.join("System32").join("taskkill.exe"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("taskkill.exe"));
        let mut command = Command::new(taskkill);
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        let status = command
            .status()
            .await
            .map_err(|error| format!("failed to run taskkill: {error}"))?;
        if !status.success() {
            return Err(format!("taskkill failed for pid {pid}"));
        }
        let pid_i32 = i32::try_from(pid).map_err(|_| "invalid process id".to_string())?;
        for _ in 0..20 {
            if !crate::unix::process_exists(pid_i32) {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        return Err(format!("process is still running after taskkill: {pid}"));
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(format!("process termination is unsupported for pid {pid}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::sync::Mutex as StdMutex;

    #[test]
    fn backoff_sequence_is_bounded_and_jittered() {
        let minimums = [800, 1_600, 4_000, 8_000, 24_000, 48_000, 96_000, 240_000];
        let maximums = [
            1_200, 2_400, 6_000, 12_000, 36_000, 72_000, 144_000, 360_000,
        ];
        for failure in 1..=12 {
            let index = (failure - 1).min(7) as usize;
            for entropy in [0, u64::MAX / 2, u64::MAX] {
                let delay = backoff_delay(failure, entropy).as_millis() as u64;
                assert!(
                    delay >= minimums[index] && delay <= maximums[index],
                    "failure {failure} produced {delay}ms"
                );
            }
        }
    }

    #[test]
    fn restart_deadline_preserves_subsecond_jitter() {
        assert_eq!(
            time_utils::parse_iso_ms(&next_restart_at(Duration::from_millis(875), 1_000)),
            Some(1_875)
        );
    }

    #[test]
    fn stable_run_resets_the_failure_sequence_before_counting_the_exit() {
        assert_eq!(
            next_consecutive_failures(7, Duration::from_secs(STABLE_RUN_SECONDS - 1)),
            8
        );
        assert_eq!(
            next_consecutive_failures(7, Duration::from_secs(STABLE_RUN_SECONDS)),
            1
        );
    }

    #[test]
    fn parses_linux_process_and_memory_samples() {
        let status = "Name:\ttest\nVmRSS:\t 1024 kB\nVmHWM:\t2048 kB\nThreads:\t7\n";
        let memory = "MemTotal: 8192 kB\nMemAvailable: 4096 kB\n";
        assert_eq!(parse_proc_kib(status, "VmRSS"), Some(1024));
        assert_eq!(parse_proc_kib(status, "VmHWM"), Some(2048));
        assert_eq!(parse_proc_number(status, "Threads"), Some(7));
        assert_eq!(parse_proc_kib(memory, "MemAvailable"), Some(4096));
        assert_eq!(parse_proc_kib(status, "Missing"), None);
    }

    #[test]
    fn parses_cgroup_v2_and_v1_memory_counters() {
        assert_eq!(
            parse_cgroup_v2_events("low 0\nhigh 2\nmax 9\noom 3\noom_kill 4\n"),
            (Some(4), Some(9))
        );
        assert_eq!(parse_cgroup_v1_failcnt(" 17\n"), Some(17));
        assert_eq!(parse_cgroup_v1_failcnt("unreadable"), None);
        assert_eq!(
            parse_cgroup_v1_oom_control("oom_kill_disable 0\nunder_oom 0\noom_kill 6\n"),
            Some(6)
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn process_resource_sampling_degrades_on_non_linux() {
        assert_eq!(sample_process_resources(std::process::id()), None);
    }

    #[test]
    fn detects_only_correlated_oom_kills() {
        let baseline = ProcessResourceSample {
            cgroup_oom_kill_count: Some(4),
            ..ProcessResourceSample::default()
        };
        let current = ProcessResourceSample {
            cgroup_oom_kill_count: Some(5),
            ..ProcessResourceSample::default()
        };
        #[cfg(unix)]
        {
            assert_eq!(
                diagnose_exit(Some(libc::SIGKILL), Some(&baseline), Some(&current)).as_deref(),
                Some("suspected cgroup OOM kill")
            );
            assert_eq!(
                diagnose_exit(Some(libc::SIGTERM), Some(&baseline), Some(&current)),
                None
            );
            assert_eq!(
                diagnose_exit(Some(libc::SIGKILL), Some(&current), Some(&current)),
                None
            );
        }
    }

    #[test]
    fn snapshot_normalization_keeps_desired_intent() {
        let snapshot = SupervisorSnapshot {
            state: SupervisorPhase::Running,
            desired_running: true,
            running: true,
            pid: None,
            ..SupervisorSnapshot::default()
        }
        .normalize();
        assert_eq!(snapshot.state, SupervisorPhase::Backoff);
        assert!(snapshot.desired_running);
        assert!(!snapshot.running);
    }

    #[test]
    fn truncates_utf8_without_breaking_characters() {
        assert_eq!(truncate_text("一二三四", 3), "一二三...");
    }

    #[cfg(unix)]
    struct TestAdapter {
        key: String,
        launch: ProcessLaunch,
        existing_pid: StdMutex<Option<u32>>,
        persist_error: StdMutex<Option<String>>,
        terminate_error: StdMutex<Option<String>>,
        snapshots: StdMutex<Vec<SupervisorSnapshot>>,
        outputs: StdMutex<Vec<String>>,
        failures: StdMutex<Vec<SupervisorFailure>>,
    }

    #[cfg(unix)]
    impl TestAdapter {
        fn shell(script: &str) -> Arc<Self> {
            Arc::new(Self {
                key: format!("test:{}", uuid::Uuid::new_v4()),
                launch: ProcessLaunch {
                    executable: "/bin/sh".into(),
                    args: vec!["-c".into(), script.into()],
                    current_dir: std::env::temp_dir(),
                },
                existing_pid: StdMutex::new(None),
                persist_error: StdMutex::new(None),
                terminate_error: StdMutex::new(None),
                snapshots: StdMutex::new(Vec::new()),
                outputs: StdMutex::new(Vec::new()),
                failures: StdMutex::new(Vec::new()),
            })
        }

        fn missing_executable() -> Arc<Self> {
            Arc::new(Self {
                key: format!("test:{}", uuid::Uuid::new_v4()),
                launch: ProcessLaunch {
                    executable: format!(
                        "/definitely/missing/fn-knock-supervisor-test-{}",
                        uuid::Uuid::new_v4()
                    )
                    .into(),
                    args: Vec::new(),
                    current_dir: std::env::temp_dir(),
                },
                existing_pid: StdMutex::new(None),
                persist_error: StdMutex::new(None),
                terminate_error: StdMutex::new(None),
                snapshots: StdMutex::new(Vec::new()),
                outputs: StdMutex::new(Vec::new()),
                failures: StdMutex::new(Vec::new()),
            })
        }

        fn adopt(pid: u32) -> Arc<Self> {
            Arc::new(Self {
                key: format!("test:{}", uuid::Uuid::new_v4()),
                launch: ProcessLaunch {
                    executable: "/bin/false".into(),
                    args: Vec::new(),
                    current_dir: std::env::temp_dir(),
                },
                existing_pid: StdMutex::new(Some(pid)),
                persist_error: StdMutex::new(None),
                terminate_error: StdMutex::new(None),
                snapshots: StdMutex::new(Vec::new()),
                outputs: StdMutex::new(Vec::new()),
                failures: StdMutex::new(Vec::new()),
            })
        }

        fn set_persist_error(&self, error: Option<&str>) {
            *self
                .persist_error
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = error.map(ToString::to_string);
        }

        fn set_terminate_error(&self, error: Option<&str>) {
            *self
                .terminate_error
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = error.map(ToString::to_string);
        }
    }

    #[cfg(unix)]
    #[async_trait]
    impl TunnelProcessAdapter for TestAdapter {
        fn key(&self) -> String {
            self.key.clone()
        }

        fn label(&self) -> String {
            "test-process".to_string()
        }

        async fn prepare_launch(&self) -> Result<ProcessLaunch, String> {
            Ok(self.launch.clone())
        }

        async fn find_existing_pid(&self) -> Option<u32> {
            *self
                .existing_pid
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        }

        async fn owns_live_pid(&self, pid: u32) -> bool {
            *self
                .existing_pid
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                == Some(pid)
                && i32::try_from(pid).is_ok_and(crate::unix::process_exists)
        }

        async fn terminate_process(&self, pid: u32) -> Result<(), String> {
            if let Some(error) = self
                .terminate_error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
            {
                return Err(error);
            }
            terminate_pid(pid).await
        }

        async fn persist_snapshot(&self, snapshot: &SupervisorSnapshot) -> Result<(), String> {
            if let Some(error) = self
                .persist_error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
            {
                return Err(error);
            }
            self.snapshots
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(snapshot.clone());
            Ok(())
        }

        async fn append_output(&self, stream: OutputStream, line: String) {
            self.outputs
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(format!("{stream:?}:{line}"));
        }

        async fn append_supervisor_log(&self, line: String) {
            self.outputs
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(line);
        }

        async fn set_expected_stop(&self, _expected: bool) {}

        async fn on_unexpected_exit(&self, _pid: Option<u32>, failure: &SupervisorFailure) {
            self.failures
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(failure.clone());
        }

        async fn remove_pid_file(&self) {}

        async fn write_pid_file(&self, _pid: u32) {}
    }

    #[cfg(unix)]
    async fn wait_for_phase(
        handle: &SupervisorHandle,
        phase: SupervisorPhase,
    ) -> SupervisorSnapshot {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let snapshot = handle.snapshot();
                if snapshot.state == phase {
                    return snapshot;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("supervisor phase timeout")
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn drains_output_and_enters_backoff_after_an_unexpected_exit() {
        let adapter = TestAdapter::shell("printf 'final-error\\n' >&2; exit 7");
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(
            adapter.clone(),
            SupervisorSnapshot::default(),
            shutdown.clone(),
        );
        assert!(handle.start().await.unwrap() > 0);
        let snapshot = wait_for_phase(&handle, SupervisorPhase::Backoff).await;
        assert!(snapshot.desired_running);
        assert!(snapshot.consecutive_failures >= 1);
        let failure = snapshot.last_failure.expect("last failure");
        assert_eq!(failure.exit_code, Some(7));
        assert_eq!(failure.recent_stderr, vec!["final-error"]);
        assert!(failure.uptime_ms < 10_000);
        handle.stop().await.unwrap();
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn first_manual_spawn_failure_does_not_arm_infinite_retry() {
        let adapter = TestAdapter::missing_executable();
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(adapter, SupervisorSnapshot::default(), shutdown.clone());
        assert!(handle.start().await.is_err());
        let snapshot = handle.snapshot();
        assert_eq!(snapshot.state, SupervisorPhase::Stopped);
        assert!(!snapshot.desired_running);
        assert_eq!(snapshot.consecutive_failures, 0);
        assert!(snapshot.next_restart_at.is_none());
        assert!(snapshot.last_failure.is_some());
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn automatic_restore_spawn_failure_keeps_retrying() {
        let adapter = TestAdapter::missing_executable();
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(
            adapter,
            SupervisorSnapshot {
                desired_running: true,
                ..SupervisorSnapshot::default()
            },
            shutdown.clone(),
        );
        let snapshot = wait_for_phase(&handle, SupervisorPhase::Backoff).await;
        assert!(snapshot.desired_running);
        assert!(snapshot.consecutive_failures >= 1);
        assert!(snapshot.next_restart_at.is_some());
        handle.stop().await.unwrap();
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn explicit_start_preempts_backoff_and_old_exit_cannot_override_new_pid() {
        let marker = std::env::temp_dir().join(format!(
            "fn-knock-supervisor-marker-{}",
            uuid::Uuid::new_v4()
        ));
        let script = format!(
            "if [ ! -e '{path}' ]; then touch '{path}'; exit 3; fi; while :; do :; done",
            path = marker.display()
        );
        let adapter = TestAdapter::shell(&script);
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(adapter, SupervisorSnapshot::default(), shutdown.clone());
        let first_pid = handle.start().await.unwrap();
        wait_for_phase(&handle, SupervisorPhase::Backoff).await;
        let second_pid = handle.start().await.unwrap();
        assert_ne!(first_pid, second_pid);
        sleep(Duration::from_millis(100)).await;
        let snapshot = handle.snapshot();
        assert_eq!(snapshot.state, SupervisorPhase::Running);
        assert_eq!(snapshot.pid, Some(second_pid));
        handle.stop().await.unwrap();
        let _ = std::fs::remove_file(marker);
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn restart_replaces_a_running_child_without_clearing_intent() {
        let adapter = TestAdapter::shell("while :; do :; done");
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(adapter, SupervisorSnapshot::default(), shutdown.clone());
        let first_pid = handle.start().await.unwrap();
        let second_pid = handle.restart().await.unwrap();
        assert_ne!(first_pid, second_pid);
        let snapshot = handle.snapshot();
        assert_eq!(snapshot.state, SupervisorPhase::Running);
        assert!(snapshot.desired_running);
        assert_eq!(snapshot.pid, Some(second_pid));
        handle.stop().await.unwrap();
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn update_pause_stops_the_child_without_clearing_intent() {
        let adapter = TestAdapter::shell("while :; do :; done");
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(adapter, SupervisorSnapshot::default(), shutdown.clone());
        let first_pid = handle.start().await.unwrap();

        handle.pause_for_restart().await.unwrap();
        let paused = handle.snapshot();
        assert_eq!(paused.state, SupervisorPhase::Stopped);
        assert!(paused.desired_running);
        assert!(!paused.running);
        assert!(paused.pid.is_none());

        let second_pid = handle.start().await.unwrap();
        assert_ne!(first_pid, second_pid);
        handle.stop().await.unwrap();
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[test]
    fn unix_exit_status_reports_signal_and_core_dump_bits() {
        use std::os::unix::process::ExitStatusExt;

        let status = ExitStatus::from_raw(libc::SIGSEGV | 0x80);
        let (_, signal, core_dumped) = exit_status_details(&status);
        assert_eq!(signal, Some(libc::SIGSEGV));
        assert!(core_dumped);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn manual_stop_cancels_guarding_without_recording_a_failure() {
        let adapter = TestAdapter::shell("while :; do :; done");
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(
            adapter.clone(),
            SupervisorSnapshot::default(),
            shutdown.clone(),
        );
        assert!(handle.start().await.unwrap() > 0);
        handle.stop().await.unwrap();
        let snapshot = handle.snapshot();
        assert_eq!(snapshot.state, SupervisorPhase::Stopped);
        assert!(!snapshot.desired_running);
        assert!(
            adapter
                .failures
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .is_empty()
        );
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn start_and_stop_require_durable_intent_before_process_changes() {
        let adapter = TestAdapter::shell("while :; do :; done");
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(
            adapter.clone(),
            SupervisorSnapshot::default(),
            shutdown.clone(),
        );
        adapter.set_persist_error(Some("storage unavailable"));
        assert!(handle.start().await.is_err());
        assert!(!handle.snapshot().desired_running);
        assert!(!handle.snapshot().running);

        adapter.set_persist_error(None);
        let pid = handle.start().await.unwrap();
        adapter.set_persist_error(Some("storage unavailable"));
        assert!(handle.stop().await.is_err());
        assert!(crate::unix::process_exists(pid as i32));
        assert!(handle.snapshot().desired_running);
        assert!(handle.snapshot().running);

        adapter.set_persist_error(None);
        handle.stop().await.unwrap();
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_termination_keeps_the_live_pid_supervised() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "while :; do :; done"])
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        let waiter = tokio::spawn(async move { child.wait().await });
        let adapter = TestAdapter::adopt(pid);
        adapter.set_terminate_error(Some("permission denied"));
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(
            adapter.clone(),
            SupervisorSnapshot {
                desired_running: true,
                ..SupervisorSnapshot::default()
            },
            shutdown.clone(),
        );
        wait_for_phase(&handle, SupervisorPhase::Running).await;
        assert!(handle.stop().await.is_err());
        let snapshot = handle.snapshot();
        assert!(snapshot.running);
        assert!(!snapshot.desired_running);
        assert_eq!(snapshot.pid, Some(pid));

        adapter.set_terminate_error(None);
        handle.stop().await.unwrap();
        let _ = waiter.await;
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn registry_keeps_supervised_instances_isolated() {
        let registry = TunnelSupervisorRegistry::default();
        let shutdown = CancellationToken::new();
        let first = registry
            .ensure(
                TestAdapter::shell("while :; do :; done"),
                SupervisorSnapshot::default(),
                shutdown.clone(),
            )
            .await;
        let second = registry
            .ensure(
                TestAdapter::shell("while :; do :; done"),
                SupervisorSnapshot::default(),
                shutdown.clone(),
            )
            .await;
        assert!(first.start().await.unwrap() > 0);
        let second_pid = second.start().await.unwrap();
        first.stop().await.unwrap();
        assert_eq!(second.snapshot().pid, Some(second_pid));
        assert!(second.snapshot().running);
        second.stop().await.unwrap();
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn registry_shutdown_waits_until_children_are_reaped() {
        let registry = TunnelSupervisorRegistry::default();
        let shutdown = CancellationToken::new();
        let adapter = TestAdapter::shell("while :; do :; done");
        let handle = registry
            .ensure(
                adapter.clone(),
                SupervisorSnapshot::default(),
                shutdown.clone(),
            )
            .await;
        let pid = handle.start().await.unwrap();
        registry.shutdown_all(Duration::from_secs(5)).await;
        assert!(!crate::unix::process_exists(pid as i32));
        assert!(registry.get(&adapter.key).await.is_none());
        let snapshot = handle.snapshot();
        assert!(!snapshot.running);
        assert!(snapshot.desired_running);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn desired_instance_adopts_a_matching_live_process() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "while :; do :; done"])
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        let waiter = tokio::spawn(async move { child.wait().await });
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(
            TestAdapter::adopt(pid),
            SupervisorSnapshot {
                desired_running: true,
                ..SupervisorSnapshot::default()
            },
            shutdown.clone(),
        );
        let snapshot = wait_for_phase(&handle, SupervisorPhase::Running).await;
        assert_eq!(snapshot.pid, Some(pid));
        assert!(!snapshot.attached);
        handle.stop().await.unwrap();
        let _ = waiter.await;
        shutdown.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_stops_the_child_but_preserves_resume_intent() {
        let adapter = TestAdapter::shell("while :; do :; done");
        let shutdown = CancellationToken::new();
        let handle = spawn_supervisor(adapter, SupervisorSnapshot::default(), shutdown.clone());
        assert!(handle.start().await.unwrap() > 0);
        shutdown.cancel();
        let snapshot = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let snapshot = handle.snapshot();
                if snapshot.state == SupervisorPhase::Stopped && !snapshot.running {
                    return snapshot;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("shutdown timeout");
        assert!(snapshot.desired_running);
    }
}
