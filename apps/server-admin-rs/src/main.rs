use std::env;

const DEFAULT_TOKIO_WORKER_THREADS: usize = 2;
const MAX_TOKIO_WORKER_THREADS: usize = 64;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(tokio_worker_threads())
        .enable_all()
        .build()?
        .block_on(server_admin_rs::app::run())
}

fn tokio_worker_threads() -> usize {
    env::var("FN_KNOCK_TOKIO_WORKER_THREADS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(MAX_TOKIO_WORKER_THREADS))
        .unwrap_or_else(default_tokio_worker_threads)
}

fn default_tokio_worker_threads() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().min(DEFAULT_TOKIO_WORKER_THREADS))
        .unwrap_or(DEFAULT_TOKIO_WORKER_THREADS)
}
