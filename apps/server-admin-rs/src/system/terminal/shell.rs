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
    async fn next_event(&mut self) -> ShellEvent;
    async fn input(&mut self, data: Vec<u8>) -> TerminalResult<()>;
    async fn resize(&mut self, cols: u32, rows: u32) -> TerminalResult<()>;
    async fn close(&mut self);
    async fn disconnect(&mut self);
}

pub(super) type BoxedShell = Box<dyn InteractiveShell>;
