use std::env;

const DEFAULT_TOKIO_WORKER_THREADS: usize = 2;
const MAX_TOKIO_WORKER_THREADS: usize = 64;
#[cfg(target_family = "unix")]
const TARGET_NOFILE_LIMIT: u64 = 1_048_576;

fn main() -> anyhow::Result<()> {
    configure_open_file_limit();
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

#[cfg(target_family = "unix")]
fn configure_open_file_limit() {
    let target = TARGET_NOFILE_LIMIT as libc::rlim_t;

    // SAFETY: getrlimit/setrlimit operate on process resource limits. This runs
    // before the async runtime starts so child tasks inherit the final value.
    unsafe {
        let target_limit = libc::rlimit {
            rlim_cur: target,
            rlim_max: target,
        };
        if libc::setrlimit(libc::RLIMIT_NOFILE, &target_limit) == 0 {
            return;
        }

        let target_error = std::io::Error::last_os_error();
        let mut inherited = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut inherited) == 0
            && inherited.rlim_cur < inherited.rlim_max
        {
            let fallback_limit = libc::rlimit {
                rlim_cur: inherited.rlim_max,
                rlim_max: inherited.rlim_max,
            };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &fallback_limit) == 0 {
                eprintln!(
                    "server-admin-rs: failed to set RLIMIT_NOFILE to {TARGET_NOFILE_LIMIT}; raised soft limit to inherited hard limit {} instead: {target_error}",
                    inherited.rlim_max
                );
                return;
            }
        }

        eprintln!(
            "server-admin-rs: failed to set RLIMIT_NOFILE to {TARGET_NOFILE_LIMIT}: {target_error}"
        );
    }
}

#[cfg(not(target_family = "unix"))]
fn configure_open_file_limit() {}

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
