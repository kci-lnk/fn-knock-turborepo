use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricReason {
    WarmingUp,
    UnsupportedPlatform,
    MissingCommand,
    PermissionDenied,
    InvalidOutput,
    CounterReset,
    Estimated,
    ExecRejected,
    Timeout,
    OutputLimit,
    CollectionFailed,
    SessionInactive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Available,
    Estimated,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricsStatus {
    Available,
    Partial,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricsPlatform {
    Linux,
    Macos,
    Unknown,
}

/// Numeric metrics use percent (CPU) or seconds (uptime), never formatted text.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalNumericMetric {
    pub value: Option<f64>,
    pub status: MetricStatus,
    pub reason: Option<MetricReason>,
}
impl TerminalNumericMetric {
    pub fn available(value: f64) -> Self {
        Self {
            value: Some(value),
            status: MetricStatus::Available,
            reason: None,
        }
    }
    pub fn unavailable(reason: MetricReason) -> Self {
        Self {
            value: None,
            status: MetricStatus::Unavailable,
            reason: Some(reason),
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCapacityMetric {
    pub used_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
    pub status: MetricStatus,
    pub reason: Option<MetricReason>,
}
impl TerminalCapacityMetric {
    pub fn available(used: u64, total: u64, percent: f64) -> Self {
        Self {
            used_bytes: Some(used),
            total_bytes: Some(total),
            percent: Some(percent),
            status: MetricStatus::Available,
            reason: None,
        }
    }
    pub fn unavailable(reason: MetricReason) -> Self {
        Self {
            used_bytes: None,
            total_bytes: None,
            percent: None,
            status: MetricStatus::Unavailable,
            reason: Some(reason),
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalMetrics {
    pub sampled_at: String,
    /// Elapsed time since collection, independent of server/browser wall clocks.
    pub sample_age_ms: u64,
    pub platform: MetricsPlatform,
    pub status: MetricsStatus,
    pub cpu: TerminalNumericMetric,
    pub memory: TerminalCapacityMetric,
    /// Capacity of the filesystem mounted at /, not all physical disks.
    pub disk: TerminalCapacityMetric,
    pub uptime: TerminalNumericMetric,
}
impl TerminalMetrics {
    pub fn unavailable(reason: MetricReason) -> Self {
        Self {
            sampled_at: crate::time_utils::now_iso(),
            sample_age_ms: 0,
            platform: MetricsPlatform::Unknown,
            status: MetricsStatus::Unavailable,
            cpu: TerminalNumericMetric::unavailable(reason),
            memory: TerminalCapacityMetric::unavailable(reason),
            disk: TerminalCapacityMetric::unavailable(reason),
            uptime: TerminalNumericMetric::unavailable(reason),
        }
    }
    pub fn update_status(&mut self) {
        let available = [
            self.cpu.status,
            self.memory.status,
            self.disk.status,
            self.uptime.status,
        ]
        .iter()
        .filter(|s| **s != MetricStatus::Unavailable)
        .count();
        self.status = match available {
            0 => MetricsStatus::Unavailable,
            4 => MetricsStatus::Available,
            _ => MetricsStatus::Partial,
        };
    }
}
