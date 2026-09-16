//! Bounded, read-only process measurements used only during explicit diagnostics.

use std::{collections::BTreeMap, time::Instant};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[cfg(target_os = "linux")]
const MAX_SMAPS_BYTES: usize = 2 * 1024 * 1024;
#[cfg(any(target_os = "linux", target_os = "netbsd", test))]
const MAX_MAPPINGS: usize = 4096;
#[cfg(any(target_os = "linux", target_os = "netbsd"))]
const MAX_THREADS: usize = 256;
const MAX_TOP_ENTRIES: usize = 8;
// Bounds on the netbsd mincore() residency walk: skip any single mapping
// larger than this, and stop walking once this many bytes have been queried
// in total, so one diagnostics call can never scan an unbounded amount of
// address space. mincore() only walks page-table residency bits (it never
// reads or writes mapped contents), so even a large span is cheap; a
// reserved-but-mostly-unfaulted thread stack alone can span 100+ MiB, so
// the per-region cap must stay well above that to avoid skipping ordinary
// mappings and reporting them as memory_maps_region_unmeasured.
#[cfg(target_os = "netbsd")]
const MAX_VMMAP_REGION_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(target_os = "netbsd")]
const MAX_VMMAP_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ResourceSample {
    pub collected_at: String,
    /// One fully occupied logical CPU is 100%; the first sample has no delta.
    #[schema(required = true)]
    pub cpu_percent: Option<f64>,
    #[schema(required = true)]
    pub rss_bytes: Option<u64>,
    #[schema(required = true)]
    pub anonymous_bytes: Option<u64>,
    #[schema(required = true)]
    pub file_bytes: Option<u64>,
    #[schema(required = true)]
    pub swap_bytes: Option<u64>,
    #[schema(required = true)]
    pub threads: Option<u64>,
    pub thread_cpu: Vec<ThreadCpuSample>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ThreadCpuSample {
    pub tid: u64,
    /// Fixed application thread labels; arbitrary OS thread names are omitted.
    pub name: String,
    pub cpu_percent: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MemoryDetailsStatus {
    Available,
    Partial,
    Unsupported,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct MemoryDetails {
    pub status: MemoryDetailsStatus,
    pub collected_at: String,
    #[schema(required = true)]
    pub rss_bytes: Option<u64>,
    #[schema(required = true)]
    pub anonymous_bytes: Option<u64>,
    #[schema(required = true)]
    pub file_bytes: Option<u64>,
    #[schema(required = true)]
    pub swap_bytes: Option<u64>,
    #[schema(required = true)]
    pub threads: Option<u64>,
    pub categories: Vec<MemoryCategory>,
    pub largest_anonymous_regions: Vec<AnonymousRegion>,
    #[schema(required = true)]
    pub allocator: Option<AllocatorStats>,
    /// Static diagnostic codes, never paths, environment values or file content.
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct MemoryCategory {
    pub category: String,
    pub mappings: u64,
    pub size_bytes: u64,
    pub rss_bytes: u64,
    pub pss_bytes: u64,
    pub anonymous_bytes: u64,
    /// `null` when the platform cannot measure this (never a fabricated zero).
    #[schema(required = true)]
    pub private_dirty_bytes: Option<u64>,
    #[schema(required = true)]
    pub swap_bytes: Option<u64>,
    #[schema(required = true)]
    pub anonymous_huge_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct AnonymousRegion {
    pub category: String,
    pub permissions: String,
    pub size_bytes: u64,
    pub rss_bytes: u64,
    pub pss_bytes: u64,
    pub anonymous_bytes: u64,
    /// `null` when the platform cannot measure this (never a fabricated zero).
    #[schema(required = true)]
    pub private_dirty_bytes: Option<u64>,
    #[schema(required = true)]
    pub swap_bytes: Option<u64>,
    #[schema(required = true)]
    pub anonymous_huge_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct AllocatorStats {
    /// Arena allocations; excludes mmap allocations and may include caches.
    /// glibc: mallinfo2 uordblks. NetBSD (jemalloc): stats.allocated.
    pub allocated_bytes: u64,
    /// Allocator-reported free arena space, not necessarily resident or
    /// releasable. glibc: mallinfo2 fordblks. NetBSD (jemalloc):
    /// stats.active minus stats.allocated (slack within active slabs).
    pub free_bytes: u64,
    pub mmap_bytes: u64,
    pub arena_bytes: u64,
    /// Best-effort estimate of space the allocator could give back to the
    /// OS; semantics are allocator-specific and not directly comparable
    /// across platforms. glibc: mallinfo2's top-most releasable estimate.
    /// NetBSD (jemalloc): resident pages that are neither active nor
    /// metadata (dirty/cached pages jemalloc could purge).
    pub releasable_bytes: u64,
}

#[derive(Default)]
struct ProcessMemory {
    rss_bytes: Option<u64>,
    anonymous_bytes: Option<u64>,
    file_bytes: Option<u64>,
    swap_bytes: Option<u64>,
    threads: Option<u64>,
}

struct CpuReading {
    at: Instant,
    identity: u64,
    seconds: f64,
    threads: BTreeMap<u64, ThreadReading>,
}

struct ThreadReading {
    identity: u64,
    seconds: f64,
    name: &'static str,
}

#[derive(Default)]
pub(crate) struct ResourceSampler {
    previous: Option<CpuReading>,
}

impl ResourceSampler {
    pub(crate) fn new() -> Self {
        Self { previous: None }
    }

    /// Blocking `/proc`/OS reads. The caller owns the sampling interval.
    pub(crate) fn sample(&mut self) -> ResourceSample {
        let mut errors = Vec::new();
        let memory = process_memory(&mut errors);
        let reading = read_cpu(&mut errors);
        let (cpu_percent, thread_cpu) = cpu_delta(self.previous.as_ref(), reading.as_ref());
        // A failed read resets the baseline rather than presenting a stale delta.
        self.previous = reading;
        ResourceSample {
            collected_at: crate::time_utils::now_iso(),
            cpu_percent,
            rss_bytes: memory.rss_bytes,
            anonymous_bytes: memory.anonymous_bytes,
            file_bytes: memory.file_bytes,
            swap_bytes: memory.swap_bytes,
            threads: memory.threads,
            thread_cpu,
            errors,
        }
    }
}

fn cpu_delta(
    previous: Option<&CpuReading>,
    current: Option<&CpuReading>,
) -> (Option<f64>, Vec<ThreadCpuSample>) {
    let (Some(previous), Some(current)) = (previous, current) else {
        return (None, Vec::new());
    };
    if previous.identity != current.identity {
        return (None, Vec::new());
    }
    let Some(elapsed) = current.at.checked_duration_since(previous.at) else {
        return (None, Vec::new());
    };
    let elapsed = elapsed.as_secs_f64();
    let Some(cpu) = percentage(previous.seconds, current.seconds, elapsed) else {
        return (None, Vec::new());
    };
    let mut threads: Vec<_> = current
        .threads
        .iter()
        .filter_map(|(tid, now)| {
            let before = previous.threads.get(tid)?;
            if before.identity != now.identity {
                return None;
            }
            let cpu_percent = percentage(before.seconds, now.seconds, elapsed)?;
            (cpu_percent > 0.0).then(|| ThreadCpuSample {
                tid: *tid,
                name: now.name.to_string(),
                cpu_percent,
            })
        })
        .collect();
    threads.sort_by(|left, right| {
        right
            .cpu_percent
            .total_cmp(&left.cpu_percent)
            .then_with(|| left.tid.cmp(&right.tid))
    });
    threads.truncate(MAX_TOP_ENTRIES);
    (Some(cpu), threads)
}

fn percentage(before: f64, after: f64, elapsed: f64) -> Option<f64> {
    if !before.is_finite()
        || !after.is_finite()
        || !elapsed.is_finite()
        || before < 0.0
        || after < before
        || elapsed <= 0.0
    {
        return None;
    }
    let value = (after - before) / elapsed * 100.0;
    value.is_finite().then_some(value)
}

fn add_error(errors: &mut Vec<String>, code: &'static str) {
    if !errors.iter().any(|error| error == code) {
        errors.push(code.to_string());
    }
}

#[cfg(any(target_os = "linux", test))]
fn read_bounded(path: &std::path::Path, limit: usize) -> std::io::Result<(String, bool)> {
    use std::io::Read;

    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() > limit;
    if truncated {
        bytes.truncate(limit);
        // Do not parse a partially read metric or header as a complete line.
        bytes.truncate(bytes.iter().rposition(|byte| *byte == b'\n').unwrap_or(0));
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

#[cfg(any(target_os = "linux", test))]
fn parse_kib(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let number = parts.next()?.parse::<u64>().ok()?;
    if parts.next()? != "kB" || parts.next().is_some() {
        return None;
    }
    number.checked_mul(1024)
}

#[cfg(any(target_os = "linux", test))]
fn parse_status(raw: &str) -> ProcessMemory {
    let mut memory = ProcessMemory::default();
    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key {
            "VmRSS" => memory.rss_bytes = parse_kib(value),
            "RssAnon" => memory.anonymous_bytes = parse_kib(value),
            "RssFile" => memory.file_bytes = parse_kib(value),
            "VmSwap" => memory.swap_bytes = parse_kib(value),
            "Threads" => memory.threads = value.trim().parse().ok(),
            _ => {}
        }
    }
    memory
}

#[cfg(target_os = "linux")]
fn process_memory(errors: &mut Vec<String>) -> ProcessMemory {
    let mut memory = match read_bounded(std::path::Path::new("/proc/self/status"), 64 * 1024) {
        Ok((raw, truncated)) => {
            if truncated {
                add_error(errors, "process_status_truncated");
            }
            parse_status(&raw)
        }
        Err(_) => {
            add_error(errors, "process_status_unavailable");
            ProcessMemory::default()
        }
    };
    if memory.rss_bytes.is_none()
        || memory.anonymous_bytes.is_none()
        || memory.file_bytes.is_none()
        || memory.swap_bytes.is_none()
        || memory.threads.is_none()
    {
        add_error(errors, "process_status_incomplete");
    }
    if memory.rss_bytes.is_none() {
        memory.rss_bytes = super::current_process_rss_bytes();
    }
    memory
}

#[cfg(target_os = "netbsd")]
fn process_memory(errors: &mut Vec<String>) -> ProcessMemory {
    // No smaps-equivalent breakdown is available; only RSS is reported.
    let rss_bytes = super::current_process_rss_bytes();
    if rss_bytes.is_none() {
        add_error(errors, "memory_breakdown_unsupported");
    }
    ProcessMemory {
        rss_bytes,
        ..ProcessMemory::default()
    }
}

#[cfg(not(any(target_os = "linux", target_os = "netbsd")))]
fn process_memory(errors: &mut Vec<String>) -> ProcessMemory {
    add_error(errors, "memory_breakdown_unsupported");
    ProcessMemory {
        rss_bytes: super::current_process_rss_bytes(),
        ..ProcessMemory::default()
    }
}

#[cfg(any(target_os = "linux", test))]
struct ProcStat {
    tid: u64,
    start_ticks: u64,
    cpu_ticks: u64,
    name: &'static str,
}

#[cfg(any(target_os = "linux", test))]
fn parse_stat(raw: &str) -> Option<ProcStat> {
    let (pid, rest) = raw.split_once('(')?;
    let end = rest.rfind(')')?;
    let comm = &rest[..end];
    let fields: Vec<_> = rest[end + 1..].split_whitespace().take(20).collect();
    Some(ProcStat {
        tid: pid.trim().parse().ok()?,
        cpu_ticks: fields
            .get(11)?
            .parse::<u64>()
            .ok()?
            .checked_add(fields.get(12)?.parse::<u64>().ok()?)?,
        start_ticks: fields.get(19)?.parse().ok()?,
        // Linux comm is mutable; only disclose labels owned by this application.
        name: match comm {
            "server-admin-rs" => "server-admin-rs",
            "tokio-rt-worker" => "tokio-rt-worker",
            "fn-knock-local-" => "local-pty-worker",
            _ => "other-thread",
        },
    })
}

#[cfg(target_os = "linux")]
fn read_cpu(errors: &mut Vec<String>) -> Option<CpuReading> {
    // SAFETY: sysconf reads a process-independent numeric clock-tick setting.
    let tick_rate = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if tick_rate <= 0 {
        add_error(errors, "cpu_tick_rate_unavailable");
        return None;
    }
    let stat = read_bounded(std::path::Path::new("/proc/self/stat"), 4096)
        .ok()
        .filter(|(_, truncated)| !truncated)
        .and_then(|(raw, _)| parse_stat(&raw));
    let Some(stat) = stat else {
        add_error(errors, "process_cpu_unavailable");
        return None;
    };
    let mut reading = CpuReading {
        at: Instant::now(),
        identity: stat.start_ticks,
        seconds: stat.cpu_ticks as f64 / tick_rate as f64,
        threads: BTreeMap::new(),
    };
    let Ok(entries) = std::fs::read_dir("/proc/self/task") else {
        add_error(errors, "thread_cpu_unavailable");
        return Some(reading);
    };
    for (index, entry) in entries.enumerate() {
        if index >= MAX_THREADS {
            add_error(errors, "thread_list_truncated");
            break;
        }
        let Ok(entry) = entry else {
            add_error(errors, "thread_cpu_incomplete");
            continue;
        };
        let Some(stat) = read_bounded(&entry.path().join("stat"), 4096)
            .ok()
            .filter(|(_, truncated)| !truncated)
            .and_then(|(raw, _)| parse_stat(&raw))
        else {
            // Threads may exit while enumerating; omit their sample entirely.
            add_error(errors, "thread_cpu_incomplete");
            continue;
        };
        reading.threads.insert(
            stat.tid,
            ThreadReading {
                identity: stat.start_ticks,
                seconds: stat.cpu_ticks as f64 / tick_rate as f64,
                name: stat.name,
            },
        );
    }
    Some(reading)
}

#[cfg(target_os = "macos")]
fn read_cpu(errors: &mut Vec<String>) -> Option<CpuReading> {
    // No per-LWP accounting is wired up yet; only process-level CPU is reported.
    add_error(errors, "thread_cpu_unsupported");
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the writable rusage buffer on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        add_error(errors, "process_cpu_unavailable");
        return None;
    }
    // SAFETY: getrusage returned success above.
    let usage = unsafe { usage.assume_init() };
    let seconds = usage.ru_utime.tv_sec as f64
        + usage.ru_stime.tv_sec as f64
        + (usage.ru_utime.tv_usec as f64 + usage.ru_stime.tv_usec as f64) / 1_000_000.0;
    Some(CpuReading {
        at: Instant::now(),
        identity: u64::from(std::process::id()),
        seconds,
        threads: BTreeMap::new(),
    })
}

#[cfg(target_os = "netbsd")]
fn read_cpu(errors: &mut Vec<String>) -> Option<CpuReading> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the writable rusage buffer on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        add_error(errors, "process_cpu_unavailable");
        return None;
    }
    // SAFETY: getrusage returned success above.
    let usage = unsafe { usage.assume_init() };
    let seconds = usage.ru_utime.tv_sec as f64
        + usage.ru_stime.tv_sec as f64
        + (usage.ru_utime.tv_usec as f64 + usage.ru_stime.tv_usec as f64) / 1_000_000.0;
    let mut reading = CpuReading {
        at: Instant::now(),
        identity: u64::from(std::process::id()),
        seconds,
        threads: BTreeMap::new(),
    };

    let Some(lwps) = read_netbsd_lwps() else {
        add_error(errors, "thread_cpu_unavailable");
        return Some(reading);
    };
    if lwps.len() > MAX_THREADS {
        add_error(errors, "thread_list_truncated");
    }
    for lwp in lwps.into_iter().take(MAX_THREADS) {
        let tid = lwp.l_lid as u64;
        reading.threads.insert(
            tid,
            ThreadReading {
                // NetBSD's kinfo_lwp has no per-LWP start-time field to guard
                // against tid reuse the way Linux's start_ticks does; the
                // monotonic-CPU-time check in `percentage()` already rejects
                // a reused tid whose rtime regressed, so identity == tid here.
                identity: tid,
                seconds: lwp.l_rtime_sec as f64 + lwp.l_rtime_usec as f64 / 1_000_000.0,
                name: netbsd_thread_name(&lwp.l_name),
            },
        );
    }
    Some(reading)
}

/// Queries `kern.lwp` for the calling process's threads. Two-pass sysctl:
/// first with a null buffer to size the result, then a real fetch sized a
/// few entries larger in case the thread count grew in between.
#[cfg(target_os = "netbsd")]
fn read_netbsd_lwps() -> Option<Vec<libc::kinfo_lwp>> {
    let elem_size = std::mem::size_of::<libc::kinfo_lwp>() as libc::c_int;
    let mut mib = [
        libc::CTL_KERN,
        libc::KERN_LWP,
        std::process::id() as libc::c_int,
        elem_size,
        0,
    ];
    let mut len: libc::size_t = 0;
    // SAFETY: `mib` is a valid 5-element kern.lwp MIB; a null `oldp` with a
    // valid `oldlenp` only queries the required buffer size.
    if unsafe {
        libc::sysctl(
            mib.as_ptr(),
            mib.len() as libc::c_uint,
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null(),
            0,
        )
    } != 0
    {
        return None;
    }
    let count = len / std::mem::size_of::<libc::kinfo_lwp>() + 4;
    mib[4] = count as libc::c_int;
    let mut buf: Vec<libc::kinfo_lwp> = Vec::with_capacity(count);
    let mut len = (count * std::mem::size_of::<libc::kinfo_lwp>()) as libc::size_t;
    // SAFETY: `buf` has capacity for `count` kinfo_lwp records and `len`
    // describes exactly that many bytes; sysctl writes at most `len` bytes
    // and reports the actual bytes written back through `len`.
    if unsafe {
        libc::sysctl(
            mib.as_ptr(),
            mib.len() as libc::c_uint,
            buf.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null(),
            0,
        )
    } != 0
    {
        return None;
    }
    let actual = len / std::mem::size_of::<libc::kinfo_lwp>();
    // SAFETY: sysctl reported `actual` complete kinfo_lwp records written.
    unsafe { buf.set_len(actual) };
    Some(buf)
}

#[cfg(target_os = "netbsd")]
fn netbsd_thread_name(raw: &[libc::c_char]) -> &'static str {
    let len = raw.iter().take_while(|byte| **byte != 0).count();
    // SAFETY: `raw[..len]` contains only the non-NUL bytes preceding the
    // terminator found above.
    let bytes: Vec<u8> = raw[..len].iter().map(|byte| *byte as u8).collect();
    // Thread names are set by this application's own code; only disclose
    // labels we recognize. KI_LNAMELEN (20 bytes) truncates the PTY worker
    // names ("fn-knock-local-pty-reader" etc.) before they reach here, so
    // match on the shared prefix rather than the full name.
    match std::str::from_utf8(&bytes).unwrap_or("") {
        "server-admin-rs" => "server-admin-rs",
        "tokio-rt-worker" => "tokio-rt-worker",
        name if name.starts_with("fn-knock-local-") => "local-pty-worker",
        _ => "other-thread",
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "netbsd")))]
fn read_cpu(errors: &mut Vec<String>) -> Option<CpuReading> {
    add_error(errors, "process_cpu_unsupported");
    add_error(errors, "thread_cpu_unsupported");
    None
}

/// Blocking and bounded. It never reads address-space contents or changes memory.
pub(crate) fn collect_memory_details() -> MemoryDetails {
    let mut errors = Vec::new();
    let memory = process_memory(&mut errors);
    let mut details = MemoryDetails {
        status: MemoryDetailsStatus::Unsupported,
        collected_at: crate::time_utils::now_iso(),
        rss_bytes: memory.rss_bytes,
        anonymous_bytes: memory.anonymous_bytes,
        file_bytes: memory.file_bytes,
        swap_bytes: memory.swap_bytes,
        threads: memory.threads,
        categories: Vec::new(),
        largest_anonymous_regions: Vec::new(),
        allocator: None,
        errors,
    };
    #[cfg(target_os = "linux")]
    {
        match read_bounded(std::path::Path::new("/proc/self/smaps"), MAX_SMAPS_BYTES) {
            Ok((raw, truncated)) => {
                let parsed = parse_smaps(&raw, truncated, MAX_MAPPINGS);
                details.categories = parsed.categories;
                details.largest_anonymous_regions = parsed.largest_anonymous_regions;
                for error in parsed.errors {
                    add_error(&mut details.errors, error);
                }
            }
            Err(_) => add_error(&mut details.errors, "memory_maps_unavailable"),
        }
        details.allocator = allocator_stats(&mut details.errors);
        details.status = if details.categories.is_empty() && details.rss_bytes.is_none() {
            MemoryDetailsStatus::Unavailable
        } else if details.errors.is_empty() {
            MemoryDetailsStatus::Available
        } else {
            MemoryDetailsStatus::Partial
        };
    }
    #[cfg(target_os = "netbsd")]
    {
        let (categories, largest_anonymous_regions) = netbsd_memory_maps(&mut details.errors);
        details.categories = categories;
        details.largest_anonymous_regions = largest_anonymous_regions;
        details.allocator = allocator_stats(&mut details.errors);
        details.status = if details.categories.is_empty() && details.rss_bytes.is_none() {
            MemoryDetailsStatus::Unavailable
        } else if details.errors.is_empty() {
            MemoryDetailsStatus::Available
        } else {
            MemoryDetailsStatus::Partial
        };
    }
    #[cfg(not(any(target_os = "linux", target_os = "netbsd")))]
    {
        add_error(&mut details.errors, "memory_maps_unsupported");
        add_error(&mut details.errors, "allocator_stats_unsupported");
    }
    details
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn allocator_stats(errors: &mut Vec<String>) -> Option<AllocatorStats> {
    // Resolve at runtime: directly linking mallinfo2 would require glibc >= 2.33
    // even on hosts that never open diagnostics. Older glibc remains supported.
    // SAFETY: RTLD_DEFAULT lookup uses a static, NUL-terminated symbol name.
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"mallinfo2".as_ptr()) };
    if symbol.is_null() {
        add_error(errors, "allocator_stats_unavailable");
        return None;
    }
    // SAFETY: glibc's mallinfo2 symbol has this exact C ABI and result layout.
    let function: unsafe extern "C" fn() -> libc::mallinfo2 =
        unsafe { std::mem::transmute(symbol) };
    // SAFETY: mallinfo2 reports allocator counters without altering allocations.
    let info = unsafe { function() };
    Some(AllocatorStats {
        allocated_bytes: info.uordblks as u64,
        free_bytes: info.fordblks as u64,
        mmap_bytes: info.hblkhd as u64,
        arena_bytes: info.arena as u64,
        releasable_bytes: info.keepcost as u64,
    })
}

#[cfg(all(target_os = "linux", not(target_env = "gnu")))]
fn allocator_stats(errors: &mut Vec<String>) -> Option<AllocatorStats> {
    add_error(errors, "allocator_stats_unsupported");
    None
}

#[cfg(target_os = "netbsd")]
unsafe extern "C" {
    // NetBSD's base libc always builds jemalloc under this prefix, so this
    // can be linked directly (unlike glibc's mallinfo2, which needs a
    // runtime version check and is looked up via dlsym instead).
    fn __je_mallctl(
        name: *const std::os::raw::c_char,
        oldp: *mut std::os::raw::c_void,
        oldlenp: *mut libc::size_t,
        newp: *mut std::os::raw::c_void,
        newlen: libc::size_t,
    ) -> std::os::raw::c_int;
}

#[cfg(target_os = "netbsd")]
fn je_stat(name: &std::ffi::CStr) -> Option<libc::size_t> {
    let mut value: libc::size_t = 0;
    let mut size = std::mem::size_of::<libc::size_t>() as libc::size_t;
    // SAFETY: `name` is a NUL-terminated mallctl key documented to accept a
    // read of a single size_t; `value`/`size` describe a writable buffer of
    // exactly that size, and no `newp` is passed so no value is set.
    let rc = unsafe {
        __je_mallctl(
            name.as_ptr(),
            (&raw mut value).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0).then_some(value)
}

#[cfg(target_os = "netbsd")]
fn allocator_stats(errors: &mut Vec<String>) -> Option<AllocatorStats> {
    // jemalloc caches its stats; "epoch" must be bumped before reading them
    // to see up-to-date numbers.
    let mut epoch: u64 = 1;
    let mut epoch_size = std::mem::size_of::<u64>() as libc::size_t;
    // SAFETY: "epoch" takes and returns a u64; `epoch`/`epoch_size` describe
    // a readable and writable 8-byte buffer for both oldp and newp.
    unsafe {
        __je_mallctl(
            c"epoch".as_ptr(),
            (&raw mut epoch).cast(),
            &mut epoch_size,
            (&raw mut epoch).cast(),
            epoch_size,
        );
    }
    let (Some(allocated), Some(active), Some(resident), Some(mapped), Some(metadata)) = (
        je_stat(c"stats.allocated"),
        je_stat(c"stats.active"),
        je_stat(c"stats.resident"),
        je_stat(c"stats.mapped"),
        je_stat(c"stats.metadata"),
    ) else {
        add_error(errors, "allocator_stats_unavailable");
        return None;
    };
    Some(AllocatorStats {
        allocated_bytes: allocated as u64,
        // Pages jemalloc's arenas currently hold but are not backing a live
        // allocation (rounding/slack within active size-class slabs).
        free_bytes: active.saturating_sub(allocated) as u64,
        mmap_bytes: mapped as u64,
        arena_bytes: active as u64,
        // Best-effort estimate of resident pages that are neither actively
        // used nor metadata, i.e. dirty/cached pages jemalloc could purge;
        // jemalloc's base stats.* namespace has no direct "releasable" key.
        releasable_bytes: resident
            .saturating_sub(active)
            .saturating_sub(metadata) as u64,
    })
}

/// Fetches the process's own VM map entries via libutil's `kinfo_getvmmap`,
/// the same sysctl(CTL_VM, VM_PROC, VM_PROC_MAP)-backed helper `pmap`-style
/// tools use. Never inspects another process; `std::process::id()` is our own pid.
#[cfg(target_os = "netbsd")]
fn read_netbsd_vmmap() -> Option<Vec<libc::kinfo_vmentry>> {
    let mut count: libc::size_t = 0;
    // SAFETY: `count` is a valid, writable size_t out-param; kinfo_getvmmap
    // either returns null or a malloc-allocated array of exactly `count`
    // initialized kinfo_vmentry records, which we copy out below and free.
    let ptr = unsafe { libc::kinfo_getvmmap(std::process::id() as libc::pid_t, &mut count) };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` was just returned non-null above alongside `count`, so it
    // points to `count` contiguous, initialized kinfo_vmentry records.
    let entries = unsafe { std::slice::from_raw_parts(ptr, count) }.to_vec();
    // SAFETY: `ptr` came from kinfo_getvmmap's internal malloc and has not
    // been freed yet; the copied-out `entries` no longer borrow from it.
    unsafe { libc::free(ptr.cast()) };
    Some(entries)
}

/// No filename or address is ever kept in the returned category: only
/// whether a backing file exists distinguishes anonymous from file mappings.
#[cfg(target_os = "netbsd")]
fn netbsd_region_category(path: &[libc::c_char]) -> &'static str {
    if path.first().copied().unwrap_or(0) == 0 {
        "anonymous_mappings"
    } else {
        "file_or_special"
    }
}

#[cfg(target_os = "netbsd")]
fn netbsd_region_permissions(entry: &libc::kinfo_vmentry) -> String {
    let protection = entry.kve_protection as libc::c_int;
    let mut permissions = String::with_capacity(4);
    permissions.push(if protection & libc::KVME_PROT_READ != 0 {
        'r'
    } else {
        '-'
    });
    permissions.push(if protection & libc::KVME_PROT_WRITE != 0 {
        'w'
    } else {
        '-'
    });
    permissions.push(if protection & libc::KVME_PROT_EXEC != 0 {
        'x'
    } else {
        '-'
    });
    // kinfo_vmentry has no direct shared/private bit; a copy-on-write entry
    // behaves like Linux's private ('p') mappings, anything else like 's'.
    let flags = entry.kve_flags as libc::c_int;
    permissions.push(if flags & libc::KVME_FLAG_COW != 0 {
        'p'
    } else {
        's'
    });
    permissions
}

// mincore(2) defines only bit 0 of each output byte ("page is resident");
// the remaining bits are reserved and NetBSD names no constant for it.
#[cfg(target_os = "netbsd")]
const NETBSD_MINCORE_RESIDENT: u8 = 0x1;

/// Counts resident pages in `[start, end)` via mincore(). Read-only: it
/// queries page-residency bits without reading or writing mapped contents.
/// Returns `None` if residency could not be measured (oversized mapping or a
/// failed/raced mincore call), which the caller must not treat as zero.
#[cfg(target_os = "netbsd")]
fn netbsd_region_resident_bytes(start: u64, end: u64, page_size: u64, budget: &mut u64) -> Option<u64> {
    let span = end.checked_sub(start).filter(|span| *span > 0)?;
    if span > MAX_VMMAP_REGION_BYTES || span > *budget {
        return None;
    }
    let pages = span.div_ceil(page_size);
    let mut vec = vec![0u8; pages as usize];
    // SAFETY: `start` is a page-aligned start address from this process's
    // own kinfo_vmentry list; `vec` provides one output byte per page across
    // exactly `span` bytes, matching what mincore requires.
    let result = unsafe {
        libc::mincore(
            start as usize as *mut libc::c_void,
            span as libc::size_t,
            vec.as_mut_ptr().cast(),
        )
    };
    if result != 0 {
        return None;
    }
    *budget -= span;
    let resident_pages = vec
        .iter()
        .filter(|byte| *byte & NETBSD_MINCORE_RESIDENT != 0)
        .count() as u64;
    Some(resident_pages * page_size)
}

#[cfg(target_os = "netbsd")]
fn netbsd_memory_maps(errors: &mut Vec<String>) -> (Vec<MemoryCategory>, Vec<AnonymousRegion>) {
    let Some(mut entries) = read_netbsd_vmmap() else {
        add_error(errors, "memory_maps_unavailable");
        return (Vec::new(), Vec::new());
    };
    if entries.len() > MAX_MAPPINGS {
        add_error(errors, "memory_maps_count_truncated");
        entries.truncate(MAX_MAPPINGS);
    }
    // SAFETY: sysconf reads a process-independent numeric clock/page setting.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        add_error(errors, "memory_maps_unavailable");
        return (Vec::new(), Vec::new());
    }
    let page_size = page_size as u64;
    let mut budget = MAX_VMMAP_TOTAL_BYTES;
    let mut region_unmeasured = false;
    let mut categories: BTreeMap<&'static str, MemoryCategory> = BTreeMap::new();
    let mut largest = Vec::<AnonymousRegion>::new();
    for entry in &entries {
        let category = netbsd_region_category(&entry.kve_path);
        let size_bytes = entry.kve_end.saturating_sub(entry.kve_start);
        let Some(rss_bytes) =
            netbsd_region_resident_bytes(entry.kve_start, entry.kve_end, page_size, &mut budget)
        else {
            // Oversized mapping, exhausted scan budget, or a raced/failed
            // mincore call: this region contributes no data at all rather
            // than a misleading zero.
            region_unmeasured = true;
            continue;
        };
        // A region's resident bytes belong to at most one sharer's "fair
        // share"; kve_ref_count approximates the object's total sharers.
        let pss_bytes = rss_bytes / u64::from(entry.kve_ref_count.max(1));
        // Regions we classified as anonymous have no backing file, so their
        // resident bytes are by construction anonymous resident bytes too.
        let anonymous_bytes = if category == "anonymous_mappings" {
            rss_bytes
        } else {
            0
        };
        let bucket = categories.entry(category).or_insert_with(|| MemoryCategory {
            category: category.to_string(),
            mappings: 0,
            size_bytes: 0,
            rss_bytes: 0,
            pss_bytes: 0,
            anonymous_bytes: 0,
            private_dirty_bytes: None,
            swap_bytes: None,
            anonymous_huge_bytes: None,
        });
        bucket.mappings += 1;
        bucket.size_bytes = bucket.size_bytes.saturating_add(size_bytes);
        bucket.rss_bytes = bucket.rss_bytes.saturating_add(rss_bytes);
        bucket.pss_bytes = bucket.pss_bytes.saturating_add(pss_bytes);
        bucket.anonymous_bytes = bucket.anonymous_bytes.saturating_add(anonymous_bytes);
        if anonymous_bytes > 0 {
            largest.push(AnonymousRegion {
                category: category.to_string(),
                permissions: netbsd_region_permissions(entry),
                size_bytes,
                rss_bytes,
                pss_bytes,
                anonymous_bytes,
                private_dirty_bytes: None,
                swap_bytes: None,
                anonymous_huge_bytes: None,
            });
            largest.sort_by_key(|region| std::cmp::Reverse(region.anonymous_bytes));
            largest.truncate(MAX_TOP_ENTRIES);
        }
    }
    if region_unmeasured {
        add_error(errors, "memory_maps_region_unmeasured");
    }
    if !entries.is_empty() {
        // Per-region dirty/swap/anon-huge accounting has no NetBSD
        // equivalent exposed via mincore(); those fields stay 0 for every
        // category/region here (never a measured true zero).
        add_error(errors, "memory_maps_incomplete");
    }
    (categories.into_values().collect(), largest)
}

#[cfg(any(target_os = "linux", test))]
#[derive(Default, Clone)]
struct MemoryAmounts {
    size: u64,
    rss: u64,
    pss: u64,
    anonymous: u64,
    private_dirty: u64,
    swap: u64,
    anonymous_huge: u64,
}

#[cfg(any(target_os = "linux", test))]
impl MemoryAmounts {
    fn accumulate(&mut self, other: &Self) {
        self.size = self.size.saturating_add(other.size);
        self.rss = self.rss.saturating_add(other.rss);
        self.pss = self.pss.saturating_add(other.pss);
        self.anonymous = self.anonymous.saturating_add(other.anonymous);
        self.private_dirty = self.private_dirty.saturating_add(other.private_dirty);
        self.swap = self.swap.saturating_add(other.swap);
        self.anonymous_huge = self.anonymous_huge.saturating_add(other.anonymous_huge);
    }
}

#[cfg(any(target_os = "linux", test))]
struct Mapping {
    category: &'static str,
    permissions: String,
    amounts: MemoryAmounts,
    fields: u8,
}

#[cfg(any(target_os = "linux", test))]
fn mapping_header(line: &str) -> Option<Mapping> {
    let mut fields = line.split_whitespace();
    let (start, end) = fields.next()?.split_once('-')?;
    if start.is_empty()
        || end.is_empty()
        || !start
            .bytes()
            .chain(end.bytes())
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    let permissions = fields.next()?;
    if permissions.len() != 4 || !permissions.bytes().all(|byte| b"rwxps-".contains(&byte)) {
        return None;
    }
    fields.next()?; // Offset: never retain addresses, file paths or inode IDs.
    fields.next()?;
    fields.next()?;
    let name = fields.next();
    let category = match name {
        Some("[heap]") => "heap",
        Some(name) if name.starts_with("[stack") => "main_stack",
        None => "anonymous_mappings",
        Some(name) if name.starts_with("[anon:") || name.starts_with("[anon_shmem:") => {
            "anonymous_mappings"
        }
        _ => "file_or_special",
    };
    Some(Mapping {
        category,
        permissions: permissions.to_string(),
        amounts: MemoryAmounts::default(),
        fields: 0,
    })
}

#[cfg(any(target_os = "linux", test))]
struct ParsedSmaps {
    categories: Vec<MemoryCategory>,
    largest_anonymous_regions: Vec<AnonymousRegion>,
    errors: Vec<&'static str>,
}

#[cfg(any(target_os = "linux", test))]
fn parse_smaps(raw: &str, truncated: bool, max_mappings: usize) -> ParsedSmaps {
    let mut categories: BTreeMap<&'static str, (u64, MemoryAmounts)> = BTreeMap::new();
    let mut largest = Vec::<AnonymousRegion>::new();
    let mut current: Option<Mapping> = None;
    let mut seen = 0usize;
    let mut incomplete = false;
    let mut invalid_metric = false;
    let mut mapping_limit_reached = false;
    let mut finish = |mapping: Mapping| {
        // These fields exist in Linux smaps; absent values must not look complete.
        if mapping.fields != 0b11_1111 {
            incomplete = true;
        }
        let category = categories.entry(mapping.category).or_default();
        category.0 += 1;
        category.1.accumulate(&mapping.amounts);
        if mapping.amounts.anonymous > 0 {
            let amount = mapping.amounts;
            largest.push(AnonymousRegion {
                category: mapping.category.to_string(),
                permissions: mapping.permissions,
                size_bytes: amount.size,
                rss_bytes: amount.rss,
                pss_bytes: amount.pss,
                anonymous_bytes: amount.anonymous,
                private_dirty_bytes: Some(amount.private_dirty),
                swap_bytes: Some(amount.swap),
                anonymous_huge_bytes: Some(amount.anonymous_huge),
            });
            largest.sort_by_key(|region| std::cmp::Reverse(region.anonymous_bytes));
            largest.truncate(MAX_TOP_ENTRIES);
        }
    };
    for line in raw.lines() {
        if let Some(mapping) = mapping_header(line) {
            if let Some(previous) = current.take() {
                finish(previous);
            }
            if seen >= max_mappings {
                mapping_limit_reached = true;
                break;
            }
            seen += 1;
            current = Some(mapping);
            continue;
        }
        let Some(mapping) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (target, bit) = match key {
            "Size" => (&mut mapping.amounts.size, 1),
            "Rss" => (&mut mapping.amounts.rss, 2),
            "Pss" => (&mut mapping.amounts.pss, 4),
            "Anonymous" => (&mut mapping.amounts.anonymous, 8),
            "Private_Dirty" => (&mut mapping.amounts.private_dirty, 16),
            "Swap" => (&mut mapping.amounts.swap, 32),
            "AnonHugePages" => (&mut mapping.amounts.anonymous_huge, 0),
            _ => continue,
        };
        if let Some(bytes) = parse_kib(value) {
            *target = bytes;
            mapping.fields |= bit;
        } else {
            invalid_metric = true;
        }
    }
    // A byte-limited read may stop halfway through its final region. Omit that
    // region instead of silently presenting incomplete values as zero.
    if !truncated && let Some(mapping) = current {
        finish(mapping);
    }
    let mut errors = Vec::new();
    if truncated {
        errors.push("memory_maps_bytes_truncated");
    }
    if mapping_limit_reached {
        errors.push("memory_maps_count_truncated");
    }
    if incomplete || invalid_metric || categories.is_empty() {
        errors.push("memory_maps_incomplete");
    }
    ParsedSmaps {
        categories: categories
            .into_iter()
            .map(|(category, (mappings, amount))| MemoryCategory {
                category: category.to_string(),
                mappings,
                size_bytes: amount.size,
                rss_bytes: amount.rss,
                pss_bytes: amount.pss,
                anonymous_bytes: amount.anonymous,
                private_dirty_bytes: Some(amount.private_dirty),
                swap_bytes: Some(amount.swap),
                anonymous_huge_bytes: Some(amount.anonymous_huge),
            })
            .collect(),
        largest_anonymous_regions: largest,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn stat(tid: u64, name: &str, user: u64, system: u64, start: u64) -> String {
        let mut fields = vec!["0".to_string(); 20];
        fields[0] = "S".to_string();
        fields[11] = user.to_string();
        fields[12] = system.to_string();
        fields[19] = start.to_string();
        format!("{tid} ({name}) {}", fields.join(" "))
    }

    fn region(index: u64, suffix: &str, anonymous_kib: u64) -> String {
        format!(
            "{index:08x}-{:08x} rw-p 00000000 00:00 0{suffix}\nSize: 2048 kB\nRss: {anonymous_kib} kB\nPss: {anonymous_kib} kB\nAnonymous: {anonymous_kib} kB\nPrivate_Dirty: {anonymous_kib} kB\nSwap: 0 kB\nAnonHugePages: 0 kB\nVmFlags: rd wr\n",
            index + 4096
        )
    }

    #[test]
    fn status_uses_kib_and_leaves_missing_or_invalid_fields_unknown() {
        let memory = parse_status(
            "VmRSS: 95848 kB\nRssAnon: 70644 kB\nRssFile: 25204 kB\nVmSwap: 0 kB\nThreads: 12\n",
        );
        assert_eq!(memory.rss_bytes, Some(95_848 * 1024));
        assert_eq!(memory.anonymous_bytes, Some(70_644 * 1024));
        assert_eq!(memory.file_bytes, Some(25_204 * 1024));
        assert_eq!(memory.swap_bytes, Some(0));
        assert_eq!(memory.threads, Some(12));
        let invalid =
            parse_status("VmRSS: 2 MB\nRssAnon: -1 kB\nRssFile: 18446744073709551615 kB\n");
        assert!(invalid.rss_bytes.is_none());
        assert!(invalid.anonymous_bytes.is_none());
        assert!(invalid.file_bytes.is_none());
        assert!(invalid.threads.is_none());
    }

    #[test]
    fn proc_stat_uses_own_cpu_and_does_not_disclose_arbitrary_thread_names() {
        let parsed = parse_stat(&stat(42, "user secret) with spaces", 120, 30, 456)).unwrap();
        assert_eq!(parsed.tid, 42);
        assert_eq!(parsed.cpu_ticks, 150);
        assert_eq!(parsed.start_ticks, 456);
        assert_eq!(parsed.name, "other-thread");
        assert_eq!(
            parse_stat(&stat(7, "tokio-rt-worker", 0, 0, 1))
                .unwrap()
                .name,
            "tokio-rt-worker"
        );
        assert!(parse_stat("42 (truncated) S 0").is_none());
        assert!(parse_stat(&stat(1, "test", u64::MAX, 1, 1)).is_none());
    }

    #[test]
    fn cpu_delta_uses_elapsed_time_and_discards_new_or_reused_threads() {
        let at = Instant::now();
        let previous = CpuReading {
            at,
            identity: 1,
            seconds: 10.0,
            threads: BTreeMap::from([
                (
                    2,
                    ThreadReading {
                        identity: 4,
                        seconds: 5.0,
                        name: "tokio-rt-worker",
                    },
                ),
                (
                    3,
                    ThreadReading {
                        identity: 5,
                        seconds: 1.0,
                        name: "other-thread",
                    },
                ),
            ]),
        };
        let current = CpuReading {
            at: at + Duration::from_millis(2000),
            identity: 1,
            seconds: 13.0,
            threads: BTreeMap::from([
                (
                    2,
                    ThreadReading {
                        identity: 4,
                        seconds: 6.0,
                        name: "tokio-rt-worker",
                    },
                ),
                (
                    3,
                    ThreadReading {
                        identity: 6,
                        seconds: 99.0,
                        name: "other-thread",
                    },
                ),
                (
                    4,
                    ThreadReading {
                        identity: 7,
                        seconds: 100.0,
                        name: "other-thread",
                    },
                ),
            ]),
        };
        assert_eq!(cpu_delta(None, Some(&current)).0, None);
        let (cpu, threads) = cpu_delta(Some(&previous), Some(&current));
        assert_eq!(cpu, Some(150.0));
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].tid, 2);
        assert_eq!(threads[0].cpu_percent, 50.0);
        let restarted = CpuReading {
            identity: 2,
            ..current
        };
        assert!(cpu_delta(Some(&previous), Some(&restarted)).0.is_none());
        assert!(percentage(10.0, 9.0, 1.0).is_none());
        assert!(percentage(1.0, 2.0, 0.0).is_none());
        assert!(percentage(1.0, f64::INFINITY, 1.0).is_none());
    }

    #[test]
    fn smaps_categories_are_bounded_and_never_expose_paths_or_addresses() {
        let raw = format!(
            "{}{}{}{}",
            region(4096, " [heap]", 100),
            region(8192, " /secret/config.db", 20),
            region(12288, " [anon:private customer name]", 50),
            region(16384, " [stack]", 4)
        );
        let result = parse_smaps(&raw, false, MAX_MAPPINGS);
        assert!(result.errors.is_empty());
        assert_eq!(result.categories.len(), 4);
        assert_eq!(
            result
                .categories
                .iter()
                .map(|value| value.rss_bytes)
                .sum::<u64>(),
            174 * 1024
        );
        assert_eq!(
            result.largest_anonymous_regions[0].anonymous_bytes,
            100 * 1024
        );
        let json = serde_json::to_string(&result.categories).unwrap();
        let regions = serde_json::to_string(&result.largest_anonymous_regions).unwrap();
        assert!(!json.contains("secret"));
        assert!(!regions.contains("private customer"));
        assert!(!regions.contains("00001000"));
    }

    #[test]
    fn smaps_truncation_omits_partial_region_and_caps_mapping_and_top_counts() {
        let raw: String = (1..=20)
            .map(|index| region(index * 4096, "", index))
            .collect();
        let full = parse_smaps(&raw, false, MAX_MAPPINGS);
        assert_eq!(full.categories[0].mappings, 20);
        assert_eq!(full.largest_anonymous_regions.len(), MAX_TOP_ENTRIES);
        assert_eq!(full.largest_anonymous_regions[0].anonymous_bytes, 20 * 1024);
        let bytes_limited = parse_smaps(&raw, true, MAX_MAPPINGS);
        assert_eq!(bytes_limited.categories[0].mappings, 19);
        assert!(
            bytes_limited
                .errors
                .contains(&"memory_maps_bytes_truncated")
        );
        let count_limited = parse_smaps(&raw, false, 3);
        assert_eq!(count_limited.categories[0].mappings, 3);
        assert_eq!(count_limited.largest_anonymous_regions.len(), 3);
        assert!(
            count_limited
                .errors
                .contains(&"memory_maps_count_truncated")
        );
        assert!(
            parse_smaps("", false, 3)
                .errors
                .contains(&"memory_maps_incomplete")
        );
        assert!(
            parse_smaps("1000-2000 rw-p 0 00:00 0\nSize: invalid\n", false, 3)
                .errors
                .contains(&"memory_maps_incomplete")
        );
    }

    #[test]
    fn bounded_read_drops_incomplete_last_line() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"first\nsecond\nthird\n").unwrap();
        let (raw, truncated) = read_bounded(file.path(), 9).unwrap();
        assert_eq!(raw, "first");
        assert!(truncated);
        let (raw, truncated) = read_bounded(file.path(), 19).unwrap();
        assert_eq!(raw, "first\nsecond\nthird\n");
        assert!(!truncated);
    }

    #[cfg(target_os = "netbsd")]
    #[test]
    fn netbsd_region_resident_bytes_matches_touched_pages() {
        // SAFETY: sysconf reads a process-independent numeric page-size setting.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        let pages = 8u64;
        let len = (page_size * pages) as libc::size_t;
        // SAFETY: a fresh, anonymous, private mapping owned solely by this
        // test; no other thread can observe or race its contents.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANON,
                -1,
                0,
            )
        };
        assert_ne!(ptr, libc::MAP_FAILED);
        // SAFETY: `ptr` is the just-created mapping of exactly `len` bytes;
        // writing to every byte forces every page to become resident.
        unsafe { std::ptr::write_bytes(ptr.cast::<u8>(), 1, len) };
        let mut budget = MAX_VMMAP_TOTAL_BYTES;
        let start = ptr as u64;
        let end = start + len as u64;
        let resident = netbsd_region_resident_bytes(start, end, page_size, &mut budget);
        // SAFETY: `ptr`/`len` describe exactly the mapping created above,
        // which nothing else references.
        unsafe { libc::munmap(ptr, len) };
        assert_eq!(resident, Some(len as u64));
    }

    #[cfg(target_os = "netbsd")]
    #[test]
    fn netbsd_memory_maps_reports_self_process() {
        let mut errors = Vec::new();
        let (categories, largest) = netbsd_memory_maps(&mut errors);
        assert!(!categories.is_empty());
        assert!(
            categories
                .iter()
                .any(|category| category.category == "file_or_special")
        );
        let total_rss: u64 = categories.iter().map(|category| category.rss_bytes).sum();
        assert!(total_rss > 0);
        // Dirty/swap/anon-huge accounting is never measured on NetBSD; the
        // caller must be told the data is partial rather than see false zeros.
        assert!(
            errors
                .iter()
                .any(|error| error == "memory_maps_incomplete")
        );
        for category in &categories {
            assert!(category.pss_bytes <= category.rss_bytes);
        }
        for region in &largest {
            assert_eq!(region.category, "anonymous_mappings");
            assert!(region.anonymous_bytes > 0);
        }
    }
}
