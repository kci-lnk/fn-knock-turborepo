use super::model::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub(in crate::system::terminal) struct CpuSnapshot {
    total: u64,
    idle: u64,
}

type Sections<'a> = HashMap<&'a str, &'a str>;
fn sections(raw: &str) -> Sections<'_> {
    raw.split("__FN_METRIC_")
        .skip(1)
        .filter_map(|part| {
            let (key, value) = part.split_once("__\n")?;
            Some((key, value.trim()))
        })
        .collect()
}
fn reason(raw: &str) -> Option<MetricReason> {
    if raw.contains("!missing_command") {
        Some(MetricReason::MissingCommand)
    } else if raw.contains("!permission_denied") {
        Some(MetricReason::PermissionDenied)
    } else if raw.contains("!collection_failed") {
        Some(MetricReason::CollectionFailed)
    } else {
        None
    }
}
fn number(raw: &str) -> Option<f64> {
    let value: f64 = raw.parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}
fn numeric(raw: &str, parsed: Option<f64>) -> TerminalNumericMetric {
    if let Some(r) = reason(raw) {
        return TerminalNumericMetric::unavailable(r);
    }
    parsed
        .map(TerminalNumericMetric::available)
        .unwrap_or_else(|| TerminalNumericMetric::unavailable(MetricReason::InvalidOutput))
}
fn capacity(raw: &str, parsed: Option<TerminalCapacityMetric>) -> TerminalCapacityMetric {
    if let Some(r) = reason(raw) {
        return TerminalCapacityMetric::unavailable(r);
    }
    parsed.unwrap_or_else(|| TerminalCapacityMetric::unavailable(MetricReason::InvalidOutput))
}
fn fields(raw: &str) -> HashMap<&str, u64> {
    raw.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((
                key,
                value
                    .split_whitespace()
                    .next()?
                    .trim_end_matches('.')
                    .parse()
                    .ok()?,
            ))
        })
        .collect()
}
fn cpu_snapshot(raw: &str) -> Option<CpuSnapshot> {
    let line = raw.lines().find(|line| line.starts_with("cpu "))?;
    // guest and guest_nice are already included in user and nice.
    let values = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if values.len() < 4 {
        return None;
    }
    Some(CpuSnapshot {
        total: values
            .iter()
            .try_fold(0_u64, |sum, v| sum.checked_add(*v))?,
        idle: values[3].checked_add(values.get(4).copied().unwrap_or(0))?,
    })
}
fn linux_cpu(raw: &str, previous: &mut Option<CpuSnapshot>) -> TerminalNumericMetric {
    let Some(current) = cpu_snapshot(raw) else {
        *previous = None;
        return numeric(raw, None);
    };
    let old = previous.replace(current);
    let Some(old) = old else {
        return TerminalNumericMetric::unavailable(MetricReason::WarmingUp);
    };
    let usage = current
        .total
        .checked_sub(old.total)
        .zip(current.idle.checked_sub(old.idle))
        .filter(|(total, idle)| *total > 0 && idle <= total)
        .map(|(total, idle)| (1.0 - idle as f64 / total as f64) * 100.0);
    usage
        .map(TerminalNumericMetric::available)
        .unwrap_or_else(|| TerminalNumericMetric::unavailable(MetricReason::CounterReset))
}
fn linux_memory(raw: &str) -> Option<TerminalCapacityMetric> {
    let f = fields(raw);
    let total = *f.get("MemTotal")?;
    if total == 0 {
        return None;
    }
    let estimated = !f.contains_key("MemAvailable");
    let available = if let Some(v) = f.get("MemAvailable") {
        *v
    } else {
        let free = *f.get("MemFree")?;
        let buffers = *f.get("Buffers")?;
        let cached = *f.get("Cached")?;
        free.checked_add(buffers)?
            .checked_add(cached)?
            .checked_add(f.get("SReclaimable").copied().unwrap_or(0))?
            .saturating_sub(f.get("Shmem").copied().unwrap_or(0))
    }
    .min(total);
    let used = total - available;
    let mut result = TerminalCapacityMetric::available(
        used.checked_mul(1024)?,
        total.checked_mul(1024)?,
        used as f64 / total as f64 * 100.0,
    );
    if estimated {
        result.status = MetricStatus::Estimated;
        result.reason = Some(MetricReason::Estimated);
    }
    Some(result)
}
fn mac_memory(raw: &str, total: &str) -> Option<TerminalCapacityMetric> {
    let total: u64 = total.trim().parse().ok()?;
    let page_size: u64 = raw
        .split_once("page size of ")?
        .1
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    if total == 0 || page_size == 0 {
        return None;
    }
    let f = fields(raw);
    let pages = f
        .get("Pages active")?
        .checked_add(*f.get("Pages wired down")?)?
        .checked_add(*f.get("Pages occupied by compressor")?)?;
    let used = pages.checked_mul(page_size)?.min(total);
    Some(TerminalCapacityMetric::available(
        used,
        total,
        used as f64 / total as f64 * 100.0,
    ))
}
fn mac_cpu(raw: &str) -> Option<f64> {
    let lines = raw
        .lines()
        .filter(|l| l.starts_with("CPU usage:"))
        .collect::<Vec<_>>();
    if lines.len() < 2 {
        return None;
    }
    let line = lines.last()?;
    let idle = line
        .split(',')
        .find(|part| part.contains("idle"))?
        .trim()
        .split('%')
        .next()?;
    let idle = number(idle)?;
    (idle <= 100.0).then_some(100.0 - idle)
}
fn mac_uptime(boot: &str, now: &str) -> Option<f64> {
    let boot = boot.split_once("sec =")?.1.split(',').next()?.trim();
    let delta = number(now.trim())? - number(boot)?;
    (delta >= 0.0).then_some(delta)
}
fn disk(raw: &str) -> Option<TerminalCapacityMetric> {
    // Locate Capacity from the right; df may wrap a long filesystem name.
    for line in raw.lines().rev() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.last() != Some(&"/") {
            continue;
        }
        let index = parts.iter().rposition(|p| p.ends_with('%'))?;
        if index < 3 {
            return None;
        }
        let percent = number(parts[index].trim_end_matches('%'))?;
        let total: u64 = parts[index - 3].parse().ok()?;
        let used: u64 = parts[index - 2].parse().ok()?;
        if total == 0 || used > total {
            return None;
        }
        return Some(TerminalCapacityMetric::available(
            used.checked_mul(1024)?,
            total.checked_mul(1024)?,
            percent.min(100.0),
        ));
    }
    None
}

pub(super) fn parse(raw: &str, previous: &mut Option<CpuSnapshot>) -> TerminalMetrics {
    let s = sections(raw);
    let get = |key| s.get(key).copied().unwrap_or("");
    let mut result = TerminalMetrics::unavailable(MetricReason::UnsupportedPlatform);
    match get("platform") {
        "Linux" => {
            result.platform = MetricsPlatform::Linux;
            result.cpu = linux_cpu(get("cpu"), previous);
            result.memory = capacity(get("memory"), linux_memory(get("memory")));
            result.uptime = numeric(
                get("uptime"),
                get("uptime").split_whitespace().next().and_then(number),
            );
        }
        "Darwin" => {
            *previous = None;
            result.platform = MetricsPlatform::Macos;
            result.cpu = numeric(get("cpu"), mac_cpu(get("cpu")));
            result.memory = capacity(get("memory"), mac_memory(get("memory"), get("total")));
            if let Some(r) = reason(get("total")) {
                result.memory = TerminalCapacityMetric::unavailable(r);
            }
            result.uptime = numeric(get("boot"), mac_uptime(get("boot"), get("now")));
            if let Some(r) = reason(get("now")) {
                result.uptime = TerminalNumericMetric::unavailable(r);
            }
        }
        _ => {
            *previous = None;
        }
    }
    result.disk = capacity(get("disk"), disk(get("disk")));
    result.update_status();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linux_cpu_excludes_guest_and_resets() {
        let mut prev = None;
        assert_eq!(
            linux_cpu("cpu 100 0 0 100 0 0 0 0 100 0", &mut prev).reason,
            Some(MetricReason::WarmingUp)
        );
        assert_eq!(
            linux_cpu("cpu 150 0 0 150 0 0 0 0 150 0", &mut prev).value,
            Some(50.0)
        );
        assert_eq!(
            linux_cpu("cpu 1 0 0 1", &mut prev).reason,
            Some(MetricReason::CounterReset)
        );
        assert_eq!(
            linux_cpu("cpu 1 0 0 1", &mut prev).reason,
            Some(MetricReason::CounterReset)
        );
    }
    #[test]
    fn memory_available_and_busybox_fallback() {
        let memory = linux_memory("MemTotal: 1000 kB\nMemAvailable: 600 kB").unwrap();
        assert_eq!(memory.used_bytes, Some(409600));
        let memory = linux_memory("MemTotal: 1000 kB\nMemFree: 100 kB\nBuffers: 100 kB\nCached: 200 kB\nSReclaimable: 50 kB\nShmem: 10 kB").unwrap();
        assert_eq!(memory.used_bytes, Some(560 * 1024));
        assert_eq!(memory.status, MetricStatus::Estimated);
        assert!(linux_memory("MemTotal: 0 kB").is_none());
        assert!(linux_memory("MemTotal: 1000 kB").is_none());
    }
    #[test]
    fn mac_pages_and_second_cpu_sample() {
        for page_size in [4096, 16384] {
            let raw = format!(
                "Mach Virtual Memory Statistics: (page size of {page_size} bytes)\nPages active: 10.\nPages wired down: 20.\nPages occupied by compressor: 5."
            );
            assert_eq!(
                mac_memory(&raw, "10000000").unwrap().used_bytes,
                Some(35 * page_size)
            );
        }
        assert_eq!(
            mac_cpu(
                "CPU usage: 10% user, 10% sys, 80% idle\nCPU usage: 25% user, 25% sys, 50% idle"
            ),
            Some(50.0)
        );
        assert!(mac_cpu("CPU usage: 10% user, 10% sys, 80% idle").is_none());
        assert_eq!(mac_uptime("{ sec = 100, usec = 0 }", "250"), Some(150.0));
    }
    #[test]
    fn df_wrapping_capacity_and_invalid_values() {
        assert_eq!(disk("Filesystem 1024-blocks Used Available Capacity Mounted on\n/very/long/device\n1000 200 600 25% /").unwrap().percent, Some(25.0));
        assert_eq!(
            disk("/dev/disk3s1 1000 100 400 20% /").unwrap().used_bytes,
            Some(102400)
        );
        assert!(disk("/dev/a 0 0 0 0% /").is_none());
        assert!(disk("/dev/a 100 20 80 NaN% /").is_none());
    }
    #[test]
    fn df_handles_percent_in_source_and_exhausted_reserved_space() {
        assert_eq!(
            disk("/dev/mapper/data% 1000 200 800 20% /")
                .unwrap()
                .percent,
            Some(20.0)
        );
        assert_eq!(
            disk("/dev/root 1000 990 -10 102% /").unwrap().percent,
            Some(100.0)
        );
    }
    #[test]
    fn partial_results_do_not_fabricate_zeroes() {
        let result = parse(
            "__FN_METRIC_platform__\nLinux\n__FN_METRIC_cpu__\n!permission_denied\n__FN_METRIC_memory__\n!missing_command\n__FN_METRIC_uptime__\n123.5 40\n__FN_METRIC_disk__\n/dev/a 100 20 80 20% /\n__FN_METRIC_end__\n",
            &mut None,
        );
        assert_eq!(result.status, MetricsStatus::Partial);
        assert_eq!(result.cpu.reason, Some(MetricReason::PermissionDenied));
        assert_eq!(result.memory.reason, Some(MetricReason::MissingCommand));
        assert_eq!(result.uptime.value, Some(123.5));
    }
}
