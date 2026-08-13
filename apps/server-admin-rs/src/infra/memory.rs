use std::time::Duration;

pub(crate) fn trim_allocated_memory() -> bool {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // glibc can keep freed startup arenas resident. Trimming after bursty work
        // lowers RSS on NAS targets without changing application state.
        // SAFETY: malloc_trim only asks glibc to release free heap pages; it does
        // not dereference Rust pointers or invalidate live allocations.
        unsafe {
            libc::malloc_trim(0);
        }
        true
    }

    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        false
    }
}

pub(crate) async fn trim_allocated_memory_after(delay: Duration) {
    tokio::time::sleep(delay).await;
    let started = std::time::Instant::now();
    if trim_allocated_memory() {
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis(),
            "trimmed allocator memory after startup synchronization"
        );
    }
}
