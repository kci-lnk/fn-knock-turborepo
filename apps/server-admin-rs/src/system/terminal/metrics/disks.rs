use super::{
    MetricReason, MetricsCollector, MetricsStatus, Sample, TerminalCapacityMetric,
    TerminalDiskUsage, TerminalDisks,
};
use async_trait::async_trait;
use regex::Regex;
use std::sync::LazyLock;
use tokio_util::sync::CancellationToken;

#[async_trait]
impl Sample for TerminalDisks {
    type State = ();
    fn unavailable(reason: MetricReason) -> Self {
        Self::unavailable(reason)
    }
    fn set_age(&mut self, age: u64) {
        self.sample_age_ms = age;
    }
    fn failed(&self) -> bool {
        self.status == MetricsStatus::Unavailable
    }
    async fn collect(
        collector: &dyn MetricsCollector,
        cancel: &CancellationToken,
        _: &mut (),
    ) -> Self {
        match collector.collect_disks(cancel).await {
            Ok(raw) => parse(&raw),
            Err(reason) => Self::unavailable(reason),
        }
    }
}

pub(super) fn parse(raw: &str) -> TerminalDisks {
    // Anchor on the numeric columns, preserving spaces and '%' in names. BSD
    // fallback output can include inode columns between Capacity and Mounted on.
    static ROW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
        r"^(.+?)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(\d+%|-)\s+(?:(?:\d+|-)\s+(?:\d+|-)\s+(?:\d+%|-)\s+)?(.+)$"
    ).expect("fixed df pattern")
    });
    let mut result = TerminalDisks::unavailable(MetricReason::InvalidOutput);
    let mut pending = String::new();
    let mut invalid = false;
    let mut reason = None;
    let Some(body) = raw
        .split_once("__FN_METRIC_disks__\n")
        .map(|(_, body)| body)
    else {
        return result;
    };
    for line in body
        .split("__FN_METRIC_end__")
        .next()
        .unwrap_or_default()
        .lines()
    {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("Filesystem") && line.contains("Mounted") {
            invalid |= !pending.is_empty();
            pending.clear();
            continue;
        }
        if line.starts_with('!') {
            reason = Some(if line == "!missing_command" {
                MetricReason::MissingCommand
            } else {
                MetricReason::CollectionFailed
            });
            continue;
        }
        // Only join a wrapped name to a line beginning with the size column.
        // A malformed line must not become a prefix of the next valid device.
        let continuation = line
            .split_whitespace()
            .next()
            .is_some_and(|word| word.parse::<u64>().is_ok());
        let joined = if !pending.is_empty() && continuation {
            format!("{} {}", std::mem::take(&mut pending), line)
        } else {
            invalid |= !pending.is_empty();
            pending.clear();
            line.to_owned()
        };
        let Some(fields) = ROW.captures(&joined) else {
            if !continuation && !joined.split_whitespace().any(|word| word.ends_with('%')) {
                pending = joined;
            } else {
                invalid = true;
            }
            continue;
        };
        let bytes = |index: usize| fields[index].parse::<u64>().ok()?.checked_mul(1024);
        let (Some(total), Some(used)) = (bytes(2), bytes(3)) else {
            invalid = true;
            continue;
        };
        let available = fields[4]
            .parse::<i128>()
            .ok()
            .and_then(|v| u64::try_from(v.max(0)).ok())
            .and_then(|v| v.checked_mul(1024));
        let percent = fields[5]
            .trim_end_matches('%')
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite());
        let capacity = match percent {
            Some(percent) if total > 0 && available.is_some() => {
                TerminalCapacityMetric::available(used, total, percent.clamp(0.0, 100.0))
            }
            _ => {
                invalid = true;
                TerminalCapacityMetric::unavailable(MetricReason::InvalidOutput)
            }
        };
        let filesystem = fields[1].to_owned();
        let mount_point = fields[6].to_owned();
        // The fallback df can repeat successful rows from a partial first run.
        result
            .disks
            .retain(|disk| disk.filesystem != filesystem || disk.mount_point != mount_point);
        result.disks.push(TerminalDiskUsage {
            filesystem,
            mount_point,
            capacity,
            available_bytes: available,
        });
    }
    invalid |= !pending.is_empty();
    result.disks.sort_by(|a, b| {
        (a.mount_point != "/", &a.mount_point, &a.filesystem).cmp(&(
            b.mount_point != "/",
            &b.mount_point,
            &b.filesystem,
        ))
    });
    result.reason = reason.or(if invalid {
        Some(MetricReason::InvalidOutput)
    } else {
        None
    });
    result.status = if result.disks.is_empty() {
        result.reason.get_or_insert(MetricReason::InvalidOutput);
        MetricsStatus::Unavailable
    } else if result.reason.is_some() {
        MetricsStatus::Partial
    } else {
        MetricsStatus::Available
    };
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(body: &str) -> TerminalDisks {
        parse(&format!("__FN_METRIC_disks__\n{body}\n__FN_METRIC_end__\n"))
    }
    #[test]
    fn portable_rows_preserve_names_and_capacity_semantics() {
        let data = sample(
            "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/long name\n 1000 400 500 45% /data\n/dev/root 1000 600 300 67% /\nserver:/my disk% 2000 500 1400 27% /Volumes/My Disk\n/dev/mac 1000 200 700 23% 10 99 10% /System/Volumes/Data",
        );
        assert_eq!(data.status, MetricsStatus::Available);
        assert_eq!(data.disks.len(), 4);
        assert_eq!(data.disks[0].mount_point, "/");
        assert_eq!(data.disks[0].capacity.percent, Some(67.0));
        let disk = data
            .disks
            .iter()
            .find(|d| d.filesystem == "server:/my disk%")
            .unwrap();
        assert_eq!(disk.mount_point, "/Volumes/My Disk");
        assert_eq!(disk.available_bytes, Some(1400 * 1024));
        assert_eq!(data.disks[1].mount_point, "/System/Volumes/Data");
    }
    #[test]
    fn partial_failures_overflows_zero_and_negative_space() {
        let data = sample(
            "/dev/root 100 101 -1 101% /\ntmpfs 0 0 0 - /empty\n/dev/bad 999999999999999999999999 1 1 1% /bad\n!collection_failed",
        );
        assert_eq!(data.status, MetricsStatus::Partial);
        assert_eq!(data.reason, Some(MetricReason::CollectionFailed));
        assert_eq!(data.disks.len(), 2);
        assert_eq!(data.disks[0].capacity.percent, Some(100.0));
        assert_eq!(data.disks[0].available_bytes, Some(0));
        assert_eq!(data.disks[1].capacity.percent, None);
        assert_eq!(
            sample("!missing_command").reason,
            Some(MetricReason::MissingCommand)
        );
        assert_eq!(sample("nonsense").status, MetricsStatus::Unavailable);
    }
    #[test]
    fn repeated_fallback_rows_are_deduplicated_but_mounts_are_not() {
        let data = sample("root 100 50 50 50% /\nroot 100 50 50 50% /\nroot 100 50 50 50% /bind");
        assert_eq!(data.disks.len(), 2);
    }
}
