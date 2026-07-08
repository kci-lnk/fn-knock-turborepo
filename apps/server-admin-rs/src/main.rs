use std::env;

const DEFAULT_TOKIO_WORKER_THREADS: usize = 2;
const MAX_TOKIO_WORKER_THREADS: usize = 64;

fn main() -> anyhow::Result<()> {
    configure_allocator_for_low_memory();
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

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn configure_allocator_for_low_memory() {
    // server-admin-rs is latency-light but memory-sensitive on NAS targets.
    // Keep glibc from creating large per-thread arenas and return bursty startup
    // allocations to the kernel more eagerly.
    // SAFETY: mallopt mutates process-wide glibc allocator tunables before the
    // Tokio runtime starts and does not access Rust-managed memory.
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 1);
        libc::mallopt(libc::M_TRIM_THRESHOLD, 128 * 1024);
        libc::mallopt(libc::M_MMAP_THRESHOLD, 128 * 1024);
        libc::mallopt(libc::M_TOP_PAD, 0);
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn configure_allocator_for_low_memory() {}
