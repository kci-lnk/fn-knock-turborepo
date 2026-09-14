use async_trait::async_trait;

use super::domain::TerminalResult;

#[derive(Debug)]
pub(super) enum ShellEvent {
    Data(Vec<u8>),
    Exited(u32),
    Signaled(String),
    Closed,
    Other,
}

#[async_trait]
pub(super) trait InteractiveShell: Send {
    fn metrics_collector(&self) -> Option<std::sync::Arc<dyn super::metrics::MetricsCollector>> {
        None
    }
    async fn next_event(&mut self) -> ShellEvent;
    async fn input(&mut self, data: Vec<u8>) -> TerminalResult<()>;
    async fn resize(&mut self, cols: u32, rows: u32) -> TerminalResult<()>;
    async fn close(&mut self);
    async fn disconnect(&mut self);
}

pub(super) type BoxedShell = Box<dyn InteractiveShell>;

/// Decorates a local shell without coupling its PTY implementation to metrics.
#[cfg(unix)]
pub(super) struct MeteredShell {
    pub shell: BoxedShell,
    pub collector: std::sync::Arc<dyn super::metrics::MetricsCollector>,
}
#[cfg(unix)]
#[async_trait]
impl InteractiveShell for MeteredShell {
    fn metrics_collector(&self) -> Option<std::sync::Arc<dyn super::metrics::MetricsCollector>> {
        Some(self.collector.clone())
    }
    async fn next_event(&mut self) -> ShellEvent {
        self.shell.next_event().await
    }
    async fn input(&mut self, data: Vec<u8>) -> TerminalResult<()> {
        self.shell.input(data).await
    }
    async fn resize(&mut self, cols: u32, rows: u32) -> TerminalResult<()> {
        self.shell.resize(cols, rows).await
    }
    async fn close(&mut self) {
        self.shell.close().await;
    }
    async fn disconnect(&mut self) {
        self.shell.disconnect().await;
    }
}
