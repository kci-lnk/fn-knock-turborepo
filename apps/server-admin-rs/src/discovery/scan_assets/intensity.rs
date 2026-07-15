use super::*;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const MIN_SAFE_CONCURRENCY: usize = 8;
const MAX_SAFE_CONCURRENCY: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScanIntensityMode {
    Auto,
    Manual,
}

impl ScanIntensityMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScanIntensityLevel {
    Low,
    Medium,
    High,
    Extreme,
}

impl ScanIntensityLevel {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Extreme => "extreme",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "extreme" => Some(Self::Extreme),
            _ => None,
        }
    }

    pub(super) fn concurrency(self) -> usize {
        match self {
            Self::Low => 32,
            Self::Medium => 115,
            Self::High => 256,
            Self::Extreme => 512,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ScanDeviceCapacity {
    pub(super) cpu_cores: usize,
    pub(super) total_memory_bytes: Option<u64>,
    pub(super) available_memory_bytes: Option<u64>,
    pub(super) file_descriptor_limit: Option<u64>,
    pub(super) safe_concurrency: usize,
}

#[derive(Clone, Debug)]
pub(super) struct ScanRuntimeSettings {
    pub(super) mode: ScanIntensityMode,
    pub(super) configured_level: ScanIntensityLevel,
    pub(super) recommended_level: ScanIntensityLevel,
    pub(super) effective_level: ScanIntensityLevel,
    pub(super) configured_concurrency: usize,
    pub(super) effective_concurrency: usize,
    pub(super) capacity: ScanDeviceCapacity,
}

pub(super) fn read_scan_intensity_config(
    config: &Value,
) -> (ScanIntensityMode, ScanIntensityLevel) {
    let settings = config.get("scan_discovery");
    let mode = settings
        .and_then(|value| value.get("intensity_mode"))
        .and_then(Value::as_str)
        .and_then(ScanIntensityMode::parse)
        .unwrap_or(ScanIntensityMode::Auto);
    let level = settings
        .and_then(|value| value.get("intensity_level"))
        .and_then(Value::as_str)
        .and_then(ScanIntensityLevel::parse)
        .unwrap_or(ScanIntensityLevel::Medium);
    (mode, level)
}

pub(super) fn resolve_scan_runtime_settings(config: &Value) -> ScanRuntimeSettings {
    let (mode, configured_level) = read_scan_intensity_config(config);
    let capacity = detect_scan_device_capacity();
    let recommended_level = recommend_scan_intensity(&capacity);
    let effective_level = if mode == ScanIntensityMode::Auto {
        recommended_level
    } else {
        configured_level
    };
    let configured_concurrency = effective_level.concurrency();
    let effective_concurrency =
        effective_concurrency_for_level(effective_level, capacity.safe_concurrency);
    ScanRuntimeSettings {
        mode,
        configured_level,
        recommended_level,
        effective_level,
        configured_concurrency,
        effective_concurrency,
        capacity,
    }
}

pub(super) fn effective_concurrency_for_level(
    level: ScanIntensityLevel,
    safe_concurrency: usize,
) -> usize {
    level.concurrency().min(safe_concurrency).max(1)
}

pub(super) fn build_discover_settings_payload(config: &Value) -> Value {
    let runtime = resolve_scan_runtime_settings(config);
    scan_runtime_settings_payload(&runtime)
}

pub(super) fn scan_runtime_settings_payload(runtime: &ScanRuntimeSettings) -> Value {
    json!({
        "intensityMode": runtime.mode.as_str(),
        "configuredLevel": runtime.configured_level.as_str(),
        "recommendedLevel": runtime.recommended_level.as_str(),
        "effectiveLevel": runtime.effective_level.as_str(),
        "configuredConcurrency": runtime.configured_concurrency,
        "effectiveConcurrency": runtime.effective_concurrency,
        "capability": {
            "cpuCores": runtime.capacity.cpu_cores,
            "totalMemoryMiB": runtime.capacity.total_memory_bytes.map(|value| value / MIB),
            "availableMemoryMiB": runtime.capacity.available_memory_bytes.map(|value| value / MIB),
            "fileDescriptorLimit": runtime.capacity.file_descriptor_limit,
            "safeConcurrency": runtime.capacity.safe_concurrency,
        }
    })
}

#[derive(Default)]
struct GlobalProbeBudgetState {
    active_probes: usize,
    next_registration: u64,
    task_limits: HashMap<u64, usize>,
}

pub(super) struct GlobalProbeBudget {
    semaphore: Arc<Semaphore>,
    maximum: usize,
    state: Mutex<GlobalProbeBudgetState>,
    changed: Notify,
}

impl GlobalProbeBudget {
    pub(super) fn new(maximum: usize) -> Self {
        let maximum = maximum.max(1);
        Self {
            semaphore: Arc::new(Semaphore::new(maximum)),
            maximum,
            state: Mutex::new(GlobalProbeBudgetState::default()),
            changed: Notify::new(),
        }
    }

    fn state(&self) -> MutexGuard<'_, GlobalProbeBudgetState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn current_limit_from(&self, state: &GlobalProbeBudgetState) -> usize {
        state
            .task_limits
            .values()
            .copied()
            .min()
            .unwrap_or(self.maximum)
            .min(self.maximum)
            .max(1)
    }

    pub(super) async fn register(
        self: &Arc<Self>,
        requested_limit: usize,
    ) -> GlobalProbeTaskRegistration {
        let registration_id = {
            let mut state = self.state();
            let registration_id = state.next_registration;
            state.next_registration = state.next_registration.wrapping_add(1);
            state
                .task_limits
                .insert(registration_id, requested_limit.clamp(1, self.maximum));
            registration_id
        };
        self.changed.notify_waiters();

        let registration = GlobalProbeTaskRegistration {
            budget: self.clone(),
            registration_id,
        };
        loop {
            let changed = self.changed.notified();
            let settled = {
                let state = self.state();
                state.active_probes <= self.current_limit_from(&state)
            };
            if settled {
                return registration;
            }
            changed.await;
        }
    }

    pub(super) async fn acquire(self: &Arc<Self>) -> Option<GlobalProbePermit> {
        loop {
            let changed = self.changed.notified();
            let slot_available = {
                let mut state = self.state();
                if state.active_probes < self.current_limit_from(&state) {
                    state.active_probes += 1;
                    true
                } else {
                    false
                }
            };
            if slot_available {
                let active_slot = GlobalActiveProbe {
                    budget: self.clone(),
                };
                let semaphore_permit = self.semaphore.clone().acquire_owned().await.ok()?;
                return Some(GlobalProbePermit {
                    _active_slot: active_slot,
                    _semaphore_permit: semaphore_permit,
                });
            }
            changed.await;
        }
    }

    #[cfg(test)]
    pub(super) fn current_limit(&self) -> usize {
        let state = self.state();
        self.current_limit_from(&state)
    }
}

pub(super) struct GlobalProbeTaskRegistration {
    budget: Arc<GlobalProbeBudget>,
    registration_id: u64,
}

impl Drop for GlobalProbeTaskRegistration {
    fn drop(&mut self) {
        self.budget
            .state()
            .task_limits
            .remove(&self.registration_id);
        self.budget.changed.notify_waiters();
    }
}

struct GlobalActiveProbe {
    budget: Arc<GlobalProbeBudget>,
}

impl Drop for GlobalActiveProbe {
    fn drop(&mut self) {
        let mut state = self.budget.state();
        state.active_probes = state.active_probes.saturating_sub(1);
        drop(state);
        self.budget.changed.notify_one();
    }
}

pub(super) struct GlobalProbePermit {
    _active_slot: GlobalActiveProbe,
    _semaphore_permit: OwnedSemaphorePermit,
}

pub(super) async fn global_scan_probe_budget(
    requested_capacity: usize,
) -> (Arc<GlobalProbeBudget>, GlobalProbeTaskRegistration) {
    let budget = DISCOVERY_GLOBAL_PROBE_BUDGET
        .get_or_init(|| Arc::new(GlobalProbeBudget::new(MAX_SAFE_CONCURRENCY)))
        .clone();
    let registration = budget.register(requested_capacity).await;
    (budget, registration)
}

pub(super) fn recommend_scan_intensity(capacity: &ScanDeviceCapacity) -> ScanIntensityLevel {
    let total = capacity.total_memory_bytes;
    let available = capacity.available_memory_bytes;
    if capacity.cpu_cores <= 1
        || total.is_some_and(|value| value < GIB)
        || available.is_some_and(|value| value < 256 * MIB)
        || capacity.safe_concurrency < ScanIntensityLevel::Medium.concurrency()
    {
        return ScanIntensityLevel::Low;
    }
    if capacity.cpu_cores >= 8
        && total.is_some_and(|value| value >= 8 * GIB)
        && available.is_some_and(|value| value >= 4 * GIB)
        && capacity.safe_concurrency >= ScanIntensityLevel::Extreme.concurrency()
    {
        return ScanIntensityLevel::Extreme;
    }
    if capacity.cpu_cores >= 4
        && total.is_some_and(|value| value >= 2 * GIB)
        && available.is_some_and(|value| value >= GIB)
        && capacity.safe_concurrency >= ScanIntensityLevel::High.concurrency()
    {
        return ScanIntensityLevel::High;
    }
    ScanIntensityLevel::Medium
}

pub(super) fn calculate_safe_concurrency(
    cpu_cores: usize,
    available_memory_bytes: Option<u64>,
    file_descriptor_limit: Option<u64>,
) -> usize {
    let mut budgets = vec![cpu_cores.max(1).saturating_mul(128)];
    if let Some(available) = available_memory_bytes {
        budgets.push((available / (4 * MIB)).try_into().unwrap_or(usize::MAX));
    }
    if let Some(limit) = file_descriptor_limit {
        budgets.push(
            (limit.saturating_sub(256) / 2)
                .try_into()
                .unwrap_or(usize::MAX),
        );
    }
    budgets
        .into_iter()
        .min()
        .unwrap_or(32)
        .clamp(MIN_SAFE_CONCURRENCY, MAX_SAFE_CONCURRENCY)
}

fn detect_scan_device_capacity() -> ScanDeviceCapacity {
    let cpu_cores = effective_cpu_cores();
    let (total_memory_bytes, available_memory_bytes) = effective_memory_bytes();
    let file_descriptor_limit = process_file_descriptor_limit();
    let safe_concurrency =
        calculate_safe_concurrency(cpu_cores, available_memory_bytes, file_descriptor_limit);
    ScanDeviceCapacity {
        cpu_cores,
        total_memory_bytes,
        available_memory_bytes,
        file_descriptor_limit,
        safe_concurrency,
    }
}

fn effective_cpu_cores() -> usize {
    let host = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(2)
        .max(1);
    #[cfg(target_os = "linux")]
    {
        if let Some(quota) = linux_cgroup_cpu_cores() {
            return host.min(quota.max(1));
        }
    }
    host
}

#[cfg(target_os = "linux")]
fn linux_cgroup_cpu_cores() -> Option<usize> {
    if let Ok(value) = std::fs::read_to_string("/sys/fs/cgroup/cpu.max") {
        let mut fields = value.split_whitespace();
        let quota = fields.next()?;
        let period = fields.next()?.parse::<u64>().ok()?;
        if quota != "max" && period > 0 {
            let quota = quota.parse::<u64>().ok()?;
            return usize::try_from(quota.div_ceil(period)).ok();
        }
    }
    let quota = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_quota_us")
        .ok()?
        .trim()
        .parse::<i64>()
        .ok()?;
    let period = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_period_us")
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    if quota <= 0 || period == 0 {
        None
    } else {
        usize::try_from((quota as u64).div_ceil(period)).ok()
    }
}

fn effective_memory_bytes() -> (Option<u64>, Option<u64>) {
    let (host_total, host_available) = host_memory_bytes();
    #[cfg(target_os = "linux")]
    {
        if let Some((limit, usage)) = linux_cgroup_memory_bytes() {
            let cgroup_available = limit.saturating_sub(usage);
            return (
                Some(host_total.map_or(limit, |value| value.min(limit))),
                Some(host_available.map_or(cgroup_available, |value| value.min(cgroup_available))),
            );
        }
    }
    (host_total, host_available)
}

#[cfg(target_os = "linux")]
fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let Ok(content) = std::fs::read_to_string("/proc/meminfo") else {
        return (None, None);
    };
    let read_kib = |name: &str| {
        content
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.saturating_mul(1024))
    };
    (read_kib("MemTotal:"), read_kib("MemAvailable:"))
}

#[cfg(target_os = "linux")]
fn linux_cgroup_memory_bytes() -> Option<(u64, u64)> {
    let parse = |path: &str| {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
    };
    if let (Some(limit), Some(usage)) = (
        parse("/sys/fs/cgroup/memory.max"),
        parse("/sys/fs/cgroup/memory.current"),
    ) {
        return Some((limit, usage));
    }
    let limit = parse("/sys/fs/cgroup/memory/memory.limit_in_bytes")?;
    let usage = parse("/sys/fs/cgroup/memory/memory.usage_in_bytes")?;
    (limit < (1_u64 << 60)).then_some((limit, usage))
}

#[cfg(windows)]
fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        (None, None)
    } else {
        (Some(status.ullTotalPhys), Some(status.ullAvailPhys))
    }
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub(super) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let total_pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    if page_size <= 0 || total_pages <= 0 {
        return (None, None);
    }
    let page_size = page_size as u64;
    let total = (total_pages as u64).saturating_mul(page_size);
    let mut statistics: libc::vm_statistics64_data_t = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    let result = unsafe {
        libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            (&raw mut statistics).cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return (Some(total), None);
    }
    let free = u64::from(statistics.free_count);
    let inactive = u64::from(statistics.inactive_count);
    let speculative = u64::from(statistics.speculative_count);
    let available_pages = free.saturating_add(inactive).saturating_add(speculative);
    (
        Some(total),
        Some(available_pages.saturating_mul(page_size).min(total)),
    )
}

#[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows)))]
fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let total_pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    if page_size <= 0 || total_pages <= 0 {
        return (None, None);
    }
    let page_size = page_size as u64;
    (Some((total_pages as u64).saturating_mul(page_size)), None)
}

#[cfg(unix)]
fn process_file_descriptor_limit() -> Option<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0 {
        // rlim_t is u64 on most supported Unix targets but c_ulong on some
        // 32-bit libc variants, so keep the checked conversion portable.
        #[allow(clippy::useless_conversion)]
        u64::try_from(limit.rlim_cur).ok()
    } else {
        None
    }
}

#[cfg(not(unix))]
fn process_file_descriptor_limit() -> Option<u64> {
    None
}
