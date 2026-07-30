#[cfg(target_os = "linux")]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let Ok(content) = std::fs::read_to_string("/proc/meminfo") else {
        return (None, None);
    };
    let read_kib = |name: &str| {
        content
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.saturating_mul(1024))
    };
    (read_kib("MemTotal:"), read_kib("MemAvailable:"))
}

#[cfg(windows)]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: MEMORYSTATUSEX is a plain C data structure for which an
    // all-zero bit pattern is valid; dwLength is initialized immediately.
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    // SAFETY: status points to writable, correctly sized MEMORYSTATUSEX
    // storage whose required dwLength field has been initialized.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        (None, None)
    } else {
        (Some(status.ullTotalPhys), Some(status.ullAvailPhys))
    }
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let (Some(page_size), Some(total_pages)) = (
        positive_sysconf(libc::_SC_PAGESIZE),
        positive_sysconf(libc::_SC_PHYS_PAGES),
    ) else {
        return (None, None);
    };
    let total = total_pages.saturating_mul(page_size);
    // SAFETY: vm_statistics64_data_t is a C structure consisting of integer
    // counters; zero is a valid initialized value for every field.
    let mut statistics: libc::vm_statistics64_data_t = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: statistics and count are valid writable out-parameters, and the
    // cast exposes exactly the C integer storage expected by host_statistics64.
    let result = unsafe {
        libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            (&raw mut statistics).cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return (Some(total), None);
    }
    let free = u64::from(statistics.free_count);
    let inactive = u64::from(statistics.inactive_count);
    let speculative = u64::from(statistics.speculative_count);
    let available_pages = free.saturating_add(inactive).saturating_add(speculative);
    (
        Some(total),
        Some(available_pages.saturating_mul(page_size).min(total)),
    )
}

#[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows)))]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let (Some(page_size), Some(total_pages)) = (
        positive_sysconf(libc::_SC_PAGESIZE),
        positive_sysconf(libc::_SC_PHYS_PAGES),
    ) else {
        return (None, None);
    };
    (Some(total_pages.saturating_mul(page_size)), None)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn positive_sysconf(name: libc::c_int) -> Option<u64> {
    // SAFETY: sysconf takes a constant selector and has no pointer arguments
    // or ownership requirements.
    let value = unsafe { libc::sysconf(name) };
    u64::try_from(value).ok().filter(|value| *value > 0)
}

#[cfg(unix)]
pub(crate) fn process_file_descriptor_limit() -> Option<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: limit is valid writable storage for getrlimit's output and the
    // resource selector is a libc-defined constant.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0 {
        // rlim_t is u64 on most supported Unix targets but c_ulong on some
        // 32-bit libc variants, so keep the checked conversion portable.
        #[allow(clippy::useless_conversion)]
        u64::try_from(limit.rlim_cur).ok()
    } else {
        None
    }
}

#[cfg(not(unix))]
pub(crate) fn process_file_descriptor_limit() -> Option<u64> {
    None
}
