use std::{fs, io, path::Path};

#[cfg(unix)]
pub(crate) fn is_root_process() -> bool {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
pub(crate) fn is_root_process() -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn send_signal(pid: i32, signal: libc::c_int) -> io::Result<()> {
    if pid <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pid must be positive",
        ));
    }
    // SAFETY: kill is called with a validated positive pid_t value and does not
    // dereference Rust-managed memory.
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
pub(crate) fn send_signal(pid: i32, signal: i32) -> io::Result<()> {
    let _ = (pid, signal);
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "signals are not supported on this platform",
    ))
}

#[cfg(unix)]
pub(crate) fn process_exists(pid: i32) -> bool {
    if pid <= 0 || u32::try_from(pid).ok() == Some(std::process::id()) {
        return false;
    }
    match send_signal(pid, 0) {
        Ok(()) => true,
        Err(error) => error.raw_os_error() == Some(libc::EPERM),
    }
}

#[cfg(windows)]
pub(crate) fn process_exists(pid: i32) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, STILL_ACTIVE},
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };

    let Ok(pid) = u32::try_from(pid) else {
        return false;
    };
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    // SAFETY: the returned handle is checked and closed on every path.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut exit_code) != 0;
        CloseHandle(handle);
        ok && exit_code == STILL_ACTIVE as u32
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn process_exists(_pid: i32) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn set_file_owner_from_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let path_c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "file path contains an interior NUL byte",
        )
    })?;
    // SAFETY: path_c is a live, nul-terminated pathname for the duration of
    // the call, and uid/gid come directly from metadata for the source file.
    if unsafe { libc::chown(path_c.as_ptr(), metadata.uid(), metadata.gid()) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
pub(crate) fn set_file_owner_from_metadata(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> io::Result<()> {
    Ok(())
}
