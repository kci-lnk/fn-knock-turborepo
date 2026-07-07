use std::time::Duration;

pub(crate) fn trim_allocated_memory() -> bool {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // glibc can keep freed startup arenas resident. Trimming after bursty work
        // lowers RSS on NAS targets without changing application state.
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

pub(crate) fn trim_allocated_memory_after(delay: Duration) {
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        if trim_allocated_memory() {
            tracing::debug!("trimmed allocator memory after startup");
        }
    });
}
