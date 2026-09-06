//! Bounded, read-only process measurements used only during explicit diagnostics.

use std::{collections::BTreeMap, time::Instant};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[cfg(target_os = "linux")]
const MAX_SMAPS_BYTES: usize = 2 * 1024 * 1024;
#[cfg(any(target_os = "linux", test))]
const MAX_MAPPINGS: usize = 4096;
#[cfg(target_os = "linux")]
const MAX_THREADS: usize = 256;
const MAX_TOP_ENTRIES: usize = 8;

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
    pub private_dirty_bytes: u64,
    pub swap_bytes: u64,
    pub anonymous_huge_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct AnonymousRegion {
    pub category: String,
    pub permissions: String,
    pub size_bytes: u64,
    pub rss_bytes: u64,
    pub pss_bytes: u64,
    pub anonymous_bytes: u64,
    pub private_dirty_bytes: u64,
    pub swap_bytes: u64,
    pub anonymous_huge_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct AllocatorStats {
    /// glibc arena allocations; excludes mmap allocations and may include caches.
    pub allocated_bytes: u64,
    /// Allocator-reported free arena space, not necessarily resident or releasable.
    pub free_bytes: u64,
    pub mmap_bytes: u64,
    pub arena_bytes: u64,
    /// glibc's top-most releasable space estimate; no reclamation is performed.
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

#[cfg(not(target_os = "linux"))]
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

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
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
    #[cfg(not(target_os = "linux"))]
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
                private_dirty_bytes: amount.private_dirty,
                swap_bytes: amount.swap,
                anonymous_huge_bytes: amount.anonymous_huge,
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
                private_dirty_bytes: amount.private_dirty,
                swap_bytes: amount.swap,
                anonymous_huge_bytes: amount.anonymous_huge,
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
}
