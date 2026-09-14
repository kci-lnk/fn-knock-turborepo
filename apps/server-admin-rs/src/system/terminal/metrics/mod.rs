//! Demand-driven, session-scoped sampling, completely separate from PTY I/O.
#[cfg(test)]
mod integration;
mod model;
mod parser;
mod transport;
pub use model::*;
#[cfg(unix)]
pub(super) use transport::LocalCollector;
pub(super) use transport::SshCollector;

use async_trait::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    time::Instant,
};
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};

pub(super) const SCRIPT: &str = include_str!("collect.sh");
pub(super) const TIMEOUT: Duration = Duration::from_secs(4);
pub(super) const OUTPUT_LIMIT: usize = 64 * 1024;
const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
const RETRY_INTERVAL: Duration = Duration::from_secs(15);

#[async_trait]
pub(super) trait MetricsCollector: Send + Sync {
    /// Implementations bound execution and output and clean up on cancellation.
    async fn collect(&self, cancel: &CancellationToken) -> Result<String, MetricReason>;
}

struct CachedSample {
    collected_at: Instant,
    retry_at: Instant,
    data: TerminalMetrics,
}
impl CachedSample {
    fn response_at(&self, now: Instant) -> TerminalMetrics {
        let mut data = self.data.clone();
        data.sample_age_ms = now
            .saturating_duration_since(self.collected_at)
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        data
    }
}

/// One owned worker per session. It sleeps on a bounded request queue, not a
/// timer: unattached/hidden terminals never initiate samples. All callers share
/// the same CPU baseline and cooldown, even when several requests arrive at once.
pub(super) struct MetricsService {
    requests: mpsc::Sender<oneshot::Sender<TerminalMetrics>>,
    cancel: CancellationToken,
    task: Mutex<Option<AbortOnDropHandle<()>>>,
}
impl MetricsService {
    pub fn start(collector: Arc<dyn MetricsCollector>, parent: &CancellationToken) -> Arc<Self> {
        let cancel = parent.child_token();
        let worker_cancel = cancel.clone();
        let (requests, mut receiver) = mpsc::channel::<oneshot::Sender<TerminalMetrics>>(16);
        let task = tokio::spawn(async move {
            let mut cache: Option<CachedSample> = None;
            let mut previous = None;
            loop {
                let response = tokio::select! {
                    biased;
                    _ = worker_cancel.cancelled() => break,
                    request = receiver.recv() => match request { Some(v) => v, None => break },
                };
                if response.is_closed() {
                    continue;
                }
                if let Some(sample) = &cache
                    && Instant::now() < sample.retry_at
                {
                    let _ = response.send(sample.response_at(Instant::now()));
                    continue;
                }
                // A hidden tab can return much later: don't label a long-term
                // average as current CPU usage after a gap in sampling.
                if cache.as_ref().is_some_and(|sample| {
                    Instant::now().saturating_duration_since(sample.collected_at) > RETRY_INTERVAL
                }) {
                    previous = None;
                }
                let data = match collector.collect(&worker_cancel).await {
                    Ok(raw) => parser::parse(&raw, &mut previous),
                    Err(reason) => {
                        previous = None;
                        TerminalMetrics::unavailable(reason)
                    }
                };
                if worker_cancel.is_cancelled() {
                    break;
                }
                let interval = if data.status == MetricsStatus::Unavailable {
                    RETRY_INTERVAL
                } else {
                    SAMPLE_INTERVAL
                };
                let collected_at = Instant::now();
                cache = Some(CachedSample {
                    collected_at,
                    retry_at: collected_at + interval,
                    data: data.clone(),
                });
                let _ = response.send(data);
            }
        });
        Arc::new(Self {
            requests,
            cancel,
            task: Mutex::new(Some(AbortOnDropHandle::new(task))),
        })
    }

    pub async fn sample(&self) -> TerminalMetrics {
        let (send, receive) = oneshot::channel();
        let result = tokio::select! {
            biased;
            _ = self.cancel.cancelled() => None,
            result = async {
                self.requests.send(send).await.ok()?;
                receive.await.ok()
            } => result,
        };
        result.unwrap_or_else(|| TerminalMetrics::unavailable(MetricReason::SessionInactive))
    }

    pub async fn stop(&self) {
        self.cancel.cancel();
        if let Some(mut task) = self.task.lock().await.take() {
            // Collectors normally finish cancellation in <250ms. Abort-on-drop
            // remains a final bound if a transport fails to finish cleanup.
            let _ = tokio::time::timeout(Duration::from_secs(1), &mut task).await;
        }
    }
}
impl Drop for MetricsService {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn cached_response_reports_monotonic_age_without_changing_sample_time() {
        let now = Instant::now();
        let sample = CachedSample {
            collected_at: now,
            retry_at: now + SAMPLE_INTERVAL,
            data: TerminalMetrics::unavailable(MetricReason::Timeout),
        };
        let response = sample.response_at(now + Duration::from_millis(4700));
        assert_eq!(response.sample_age_ms, 4700);
        assert_eq!(response.sampled_at, sample.data.sampled_at);
    }
    struct CountingCollector(AtomicUsize);
    #[async_trait]
    impl MetricsCollector for CountingCollector {
        async fn collect(&self, _: &CancellationToken) -> Result<String, MetricReason> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(
                "__FN_METRIC_platform__\nLinux\n__FN_METRIC_uptime__\n42 0\n__FN_METRIC_end__\n"
                    .into(),
            )
        }
    }
    #[tokio::test]
    async fn concurrent_viewers_share_sample_and_worker_stops() {
        let collector = Arc::new(CountingCollector(AtomicUsize::new(0)));
        let service = MetricsService::start(collector.clone(), &CancellationToken::new());
        assert_eq!(collector.0.load(Ordering::SeqCst), 0);
        let (a, b) = tokio::join!(service.sample(), service.sample());
        assert_eq!(a.uptime.value, Some(42.0));
        assert_eq!(a.sampled_at, b.sampled_at);
        assert_eq!(collector.0.load(Ordering::SeqCst), 1);
        service.stop().await;
        assert_eq!(
            service.sample().await.cpu.reason,
            Some(MetricReason::SessionInactive)
        );
    }
    struct BlockingCollector;
    #[async_trait]
    impl MetricsCollector for BlockingCollector {
        async fn collect(&self, cancel: &CancellationToken) -> Result<String, MetricReason> {
            cancel.cancelled().await;
            Err(MetricReason::SessionInactive)
        }
    }
    #[tokio::test]
    async fn shutdown_cancels_inflight_sample() {
        let service = MetricsService::start(Arc::new(BlockingCollector), &CancellationToken::new());
        let s = service.clone();
        let request = tokio::spawn(async move { s.sample().await });
        tokio::task::yield_now().await;
        service.stop().await;
        assert_eq!(request.await.unwrap().status, MetricsStatus::Unavailable);
    }
}
