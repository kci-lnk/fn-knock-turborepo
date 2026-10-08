use super::*;

pub(crate) type RedisResult<T> = StorageResult<T>;
#[allow(dead_code)]
pub(crate) type RedisError = StorageError;

#[allow(dead_code)]
pub(crate) trait AsyncCommands {}

#[derive(Clone)]
pub(crate) struct ConnectionManager {
    pub(super) db: Connection,
    pub(super) analytics_db: Connection,
    pub(super) auth_read_db: Connection,
    pub(super) auth_read_pool: Option<Arc<super::auth_read_pool::AuthReadPool>>,
    pub(super) health_db: Connection,
    pub(super) checkpoint_gate: Arc<RwLock<()>>,
    pub(super) primary_admission: Arc<Semaphore>,
    pub(super) analytics_admission: Arc<Semaphore>,
    pub(super) auth_read_admission: Arc<Semaphore>,
    pub(super) health_admission: Arc<Semaphore>,
    pub(super) primary_metrics: Arc<PrimaryExecutorMetrics>,
    pub(super) operation_recorder: Arc<crate::runtime_health::operations::OperationRecorder>,
    #[cfg(test)]
    pub(super) path: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PrimaryQueueStatus {
    pub(crate) queue_depth: u64,
    pub(crate) queue_depth_peak: u64,
    /// Age of the oldest caller still waiting for primary admission.
    pub(crate) queue_wait_ms: u64,
    pub(crate) queue_wait_peak_ms: u64,
    pub(crate) active_operation_ms: u64,
    pub(crate) canceled_operations: u64,
}

#[derive(Default)]
pub(super) struct PrimaryExecutorMetrics {
    state: std::sync::Mutex<PrimaryExecutorState>,
}

#[derive(Default)]
struct PrimaryExecutorState {
    waiters: BTreeMap<u64, Instant>,
    next_waiter: u64,
    active_since: Option<Instant>,
    queue_depth_peak: u64,
    queue_wait_peak_ms: u64,
    canceled_operations: u64,
}

impl PrimaryExecutorMetrics {
    fn lock(&self) -> std::sync::MutexGuard<'_, PrimaryExecutorState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn begin_wait(self: &Arc<Self>) -> PrimaryQueueWaiter {
        let mut state = self.lock();
        let id = state.next_waiter;
        state.next_waiter += 1;
        let started = Instant::now();
        state.waiters.insert(id, started);
        state.queue_depth_peak = state.queue_depth_peak.max(state.waiters.len() as u64);
        PrimaryQueueWaiter {
            metrics: self.clone(),
            id,
            admitted: false,
        }
    }

    pub(super) fn begin_execution(self: &Arc<Self>, wait_ms: u64) -> PrimaryExecution {
        let mut state = self.lock();
        state.queue_wait_peak_ms = state.queue_wait_peak_ms.max(wait_ms);
        state.active_since = Some(Instant::now());
        PrimaryExecution {
            metrics: self.clone(),
        }
    }

    pub(super) fn status(&self) -> PrimaryQueueStatus {
        let mut state = self.lock();
        // Measure the oldest request still waiting, not the previous operation's
        // admission delay. An old slow admission must not poison fresh traffic.
        let queue_wait_ms = state
            .waiters
            .first_key_value()
            .map_or(0, |(_, started)| started.elapsed().as_millis() as u64);
        state.queue_wait_peak_ms = state.queue_wait_peak_ms.max(queue_wait_ms);
        PrimaryQueueStatus {
            queue_depth: state.waiters.len() as u64,
            queue_depth_peak: state.queue_depth_peak,
            queue_wait_ms,
            queue_wait_peak_ms: state.queue_wait_peak_ms,
            active_operation_ms: state
                .active_since
                .map_or(0, |started| started.elapsed().as_millis() as u64),
            canceled_operations: state.canceled_operations,
        }
    }
}

pub(super) struct PrimaryQueueWaiter {
    metrics: Arc<PrimaryExecutorMetrics>,
    id: u64,
    admitted: bool,
}

impl PrimaryQueueWaiter {
    pub(super) fn admit(mut self) -> u64 {
        self.admitted = true;
        let mut state = self.metrics.lock();
        let wait_ms = state
            .waiters
            .remove(&self.id)
            .map_or(0, |started| started.elapsed().as_millis() as u64);
        state.queue_wait_peak_ms = state.queue_wait_peak_ms.max(wait_ms);
        wait_ms
    }
}

impl Drop for PrimaryQueueWaiter {
    fn drop(&mut self) {
        if self.admitted {
            return;
        }
        let mut state = self.metrics.lock();
        if let Some(started) = state.waiters.remove(&self.id) {
            state.queue_wait_peak_ms = state
                .queue_wait_peak_ms
                .max(started.elapsed().as_millis() as u64);
        }
        state.canceled_operations += 1;
    }
}

pub(super) struct PrimaryExecution {
    metrics: Arc<PrimaryExecutorMetrics>,
}

impl Drop for PrimaryExecution {
    fn drop(&mut self) {
        self.metrics.lock().active_since = None;
    }
}

impl AsyncCommands for ConnectionManager {}

#[cfg(test)]
mod metrics_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn old_admission_delay_does_not_mark_fresh_waiters_as_saturated() {
        let metrics = Arc::new(PrimaryExecutorMetrics::default());
        let old = metrics.begin_wait();
        metrics
            .lock()
            .waiters
            .insert(old.id, Instant::now() - Duration::from_secs(60));
        let execution = metrics.begin_execution(old.admit());
        let fresh = metrics.begin_wait();
        let status = metrics.status();
        assert_eq!(status.queue_depth, 1);
        assert!(status.queue_wait_ms < 2_000);
        assert!(status.active_operation_ms < 2_000);
        assert!(status.queue_wait_peak_ms >= 60_000);
        drop(fresh);
        drop(execution);
        let idle = metrics.status();
        assert_eq!(idle.queue_wait_ms, 0);
        assert_eq!(idle.active_operation_ms, 0);
    }

    #[test]
    fn canceling_oldest_waiter_updates_current_wait_and_retains_peak() {
        let metrics = Arc::new(PrimaryExecutorMetrics::default());
        let oldest = metrics.begin_wait();
        metrics
            .lock()
            .waiters
            .insert(oldest.id, Instant::now() - Duration::from_secs(30));
        let next = metrics.begin_wait();
        assert!(metrics.status().queue_wait_ms >= 30_000);
        drop(oldest);
        let status = metrics.status();
        assert_eq!(status.queue_depth, 1);
        assert_eq!(status.queue_depth_peak, 2);
        assert_eq!(status.canceled_operations, 1);
        assert!(status.queue_wait_ms < 2_000);
        assert!(status.queue_wait_peak_ms >= 30_000);
        next.admit();
        assert_eq!(metrics.status().queue_wait_ms, 0);
    }
}

pub(crate) mod streams {
    use super::*;

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamReadOptions {
        pub(crate) count: Option<usize>,
    }

    impl StreamReadOptions {
        pub(crate) fn count(mut self, count: usize) -> Self {
            self.count = Some(count);
            self
        }
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamRangeReply {
        pub(crate) ids: Vec<StreamId>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamReadReply {
        pub(crate) keys: Vec<StreamKey>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamKey {
        #[allow(dead_code)]
        pub(crate) key: String,
        pub(crate) ids: Vec<StreamId>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamId {
        pub(crate) id: String,
        fields: HashMap<String, String>,
    }

    impl StreamId {
        pub(crate) fn new(id: String, fields: HashMap<String, String>) -> Self {
            Self { id, fields }
        }

        pub(crate) fn get<T: FromStreamField>(&self, field: &str) -> Option<T> {
            self.fields
                .get(field)
                .and_then(|value| T::from_field(value))
        }
    }

    pub(crate) trait FromStreamField: Sized {
        fn from_field(value: &str) -> Option<Self>;
    }

    impl FromStreamField for String {
        fn from_field(value: &str) -> Option<Self> {
            Some(value.to_string())
        }
    }
}

pub(crate) enum CmdOutput {
    Nil,
    Int(i64),
    String(String),
    OptionalString(Option<String>),
    Strings(Vec<String>),
    OptionalStrings(Vec<Option<String>>),
    StringPairs(Vec<String>),
    ZPairs(Vec<(String, f64)>),
    StreamEntries(Vec<(String, Vec<String>)>),
    Scan(String, Vec<String>),
    Ints(Vec<i64>),
}

pub(crate) trait FromCmdOutput: Sized {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self>;
}

impl FromCmdOutput for () {
    fn from_cmd_output(_: CmdOutput) -> RedisResult<Self> {
        Ok(())
    }
}

impl FromCmdOutput for i64 {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Int(value) => Ok(value),
            _ => Err(storage_error("unexpected integer command result")),
        }
    }
}

impl FromCmdOutput for usize {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Int(value) => Ok(value.max(0) as usize),
            _ => Err(storage_error("unexpected usize command result")),
        }
    }
}

impl FromCmdOutput for String {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::String(value) => Ok(value),
            CmdOutput::OptionalString(Some(value)) => Ok(value),
            _ => Err(storage_error("unexpected string command result")),
        }
    }
}

impl FromCmdOutput for Option<String> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::OptionalString(value) => Ok(value),
            CmdOutput::String(value) => Ok(Some(value)),
            CmdOutput::Nil => Ok(None),
            _ => Err(storage_error("unexpected optional string command result")),
        }
    }
}

impl FromCmdOutput for Vec<String> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Strings(value) | CmdOutput::StringPairs(value) => Ok(value),
            _ => Err(storage_error("unexpected string vector command result")),
        }
    }
}

impl FromCmdOutput for Vec<Option<String>> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::OptionalStrings(value) => Ok(value),
            _ => Err(storage_error(
                "unexpected optional string vector command result",
            )),
        }
    }
}

impl FromCmdOutput for Vec<i64> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Ints(value) => Ok(value),
            _ => Err(storage_error("unexpected integer vector command result")),
        }
    }
}

impl FromCmdOutput for (String, Vec<String>) {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Scan(cursor, keys) => Ok((cursor, keys)),
            _ => Err(storage_error("unexpected scan command result")),
        }
    }
}

impl FromCmdOutput for Vec<(String, f64)> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::ZPairs(value) => Ok(value),
            _ => Err(storage_error("unexpected zset pair command result")),
        }
    }
}

impl FromCmdOutput for Vec<(String, Vec<String>)> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::StreamEntries(value) => Ok(value),
            _ => Err(storage_error("unexpected stream command result")),
        }
    }
}

pub(crate) trait FromPipeOutput: Sized {
    fn from_pipe_outputs(outputs: Vec<CmdOutput>) -> RedisResult<Self>;
}

impl FromPipeOutput for () {
    fn from_pipe_outputs(_: Vec<CmdOutput>) -> RedisResult<Self> {
        Ok(())
    }
}

impl FromPipeOutput for Vec<i64> {
    fn from_pipe_outputs(outputs: Vec<CmdOutput>) -> RedisResult<Self> {
        outputs
            .into_iter()
            .map(|output| match output {
                CmdOutput::Int(value) => Ok(value),
                _ => Err(storage_error("unexpected pipeline integer result")),
            })
            .collect()
    }
}

pub(crate) trait FromOptionalString: Sized {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self>;
}

impl FromOptionalString for Option<String> {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self> {
        Ok(value)
    }
}

impl FromOptionalString for String {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self> {
        value.ok_or_else(|| storage_error("missing string value"))
    }
}

pub(crate) trait FromDeleteCount: Sized {
    fn from_delete_count(value: usize) -> Self;
}

impl FromDeleteCount for () {
    fn from_delete_count(_: usize) -> Self {}
}

impl FromDeleteCount for usize {
    fn from_delete_count(value: usize) -> Self {
        value
    }
}

impl FromDeleteCount for i64 {
    fn from_delete_count(value: usize) -> Self {
        value as i64
    }
}

pub(crate) trait IntoKey {
    fn into_key(self) -> String;
}

impl IntoKey for &str {
    fn into_key(self) -> String {
        self.to_string()
    }
}

impl IntoKey for String {
    fn into_key(self) -> String {
        self
    }
}

impl IntoKey for &String {
    fn into_key(self) -> String {
        self.clone()
    }
}

pub(crate) trait IntoKeys {
    fn into_keys(self) -> Vec<String>;
}

impl<T: IntoKey> IntoKeys for T {
    fn into_keys(self) -> Vec<String> {
        vec![self.into_key()]
    }
}

impl IntoKeys for &[String] {
    fn into_keys(self) -> Vec<String> {
        self.to_vec()
    }
}

impl IntoKeys for &Vec<String> {
    fn into_keys(self) -> Vec<String> {
        self.clone()
    }
}

impl IntoKeys for Vec<String> {
    fn into_keys(self) -> Vec<String> {
        self
    }
}

impl<const N: usize> IntoKeys for &[&str; N] {
    fn into_keys(self) -> Vec<String> {
        self.iter().map(|value| (*value).to_string()).collect()
    }
}

pub(crate) trait IntoMembers {
    fn into_members(self) -> Vec<String>;
}

impl<T: IntoKey> IntoMembers for T {
    fn into_members(self) -> Vec<String> {
        vec![self.into_key()]
    }
}

impl IntoMembers for &[String] {
    fn into_members(self) -> Vec<String> {
        self.to_vec()
    }
}

impl IntoMembers for &Vec<String> {
    fn into_members(self) -> Vec<String> {
        self.clone()
    }
}

impl IntoMembers for Vec<String> {
    fn into_members(self) -> Vec<String> {
        self
    }
}

pub(crate) trait ToRedisArgs {
    fn append_args(&self, args: &mut Vec<String>);
}

macro_rules! impl_display_arg {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ToRedisArgs for $ty {
                fn append_args(&self, args: &mut Vec<String>) {
                    args.push(self.to_string());
                }
            }
        )*
    };
}

impl_display_arg!(i64, i32, isize, usize, u64, u32, f64);

impl ToRedisArgs for &str {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push((*self).to_string());
    }
}

impl ToRedisArgs for String {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push(self.clone());
    }
}

impl ToRedisArgs for &String {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push((*self).clone());
    }
}

impl ToRedisArgs for &[String] {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for &Vec<String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for Vec<String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for Vec<&String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().map(|value| (*value).clone()));
    }
}

impl<const N: usize> ToRedisArgs for &[&str; N] {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().map(|value| (*value).to_string()));
    }
}
