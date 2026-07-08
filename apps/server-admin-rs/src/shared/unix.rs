use std::{io, path::PathBuf};

#[cfg(unix)]
const MAX_GETPWUID_BUFFER_SIZE: usize = 1024 * 1024;

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

pub(crate) fn process_exists(pid: i32) -> bool {
    if pid <= 0 || u32::try_from(pid).ok() == Some(std::process::id()) {
        return false;
    }
    match send_signal(pid, 0) {
        Ok(()) => true,
        #[cfg(unix)]
        Err(error) => error.raw_os_error() == Some(libc::EPERM),
        #[cfg(not(unix))]
        Err(_) => false,
    }
}

#[cfg(unix)]
pub(crate) fn current_user_home_dir() -> Option<PathBuf> {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    let uid = unsafe { libc::geteuid() };
    let mut buffer_size = getpwuid_buffer_size();

    loop {
        let mut passwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        let mut buffer = vec![0_u8; buffer_size];

        // SAFETY: passwd points to valid uninitialized storage, buffer is a
        // writable byte buffer for libc, and result is a valid out-pointer.
        let code = unsafe {
            libc::getpwuid_r(
                uid,
                passwd.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };

        if code == libc::ERANGE {
            let Some(next_buffer_size) = next_getpwuid_buffer_size(buffer_size) else {
                return None;
            };
            buffer_size = next_buffer_size;
            continue;
        }
        if code != 0 || result.is_null() {
            return None;
        }

        // SAFETY: getpwuid_r returned success with a non-null result, so passwd
        // has been fully initialized and pw_dir points into buffer for this scope.
        let passwd = unsafe { passwd.assume_init() };
        if passwd.pw_dir.is_null() {
            return None;
        }
        // SAFETY: pw_dir is a nul-terminated C string provided by getpwuid_r.
        let value = unsafe { std::ffi::CStr::from_ptr(passwd.pw_dir) }
            .to_string_lossy()
            .trim()
            .to_string();
        return (!value.is_empty()).then(|| PathBuf::from(value));
    }
}

#[cfg(unix)]
fn getpwuid_buffer_size() -> usize {
    // SAFETY: sysconf has no pointer arguments and does not access Rust memory.
    let value = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value.clamp(1024, MAX_GETPWUID_BUFFER_SIZE))
        .unwrap_or(16 * 1024)
}

#[cfg(unix)]
fn next_getpwuid_buffer_size(current: usize) -> Option<usize> {
    if current >= MAX_GETPWUID_BUFFER_SIZE {
        None
    } else {
        Some(
            current
                .saturating_mul(2)
                .clamp(current + 1, MAX_GETPWUID_BUFFER_SIZE),
        )
    }
}

#[cfg(not(unix))]
pub(crate) fn current_user_home_dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[cfg(unix)]
    #[test]
    fn getpwuid_buffer_growth_retries_at_maximum_size() {
        assert_eq!(
            next_getpwuid_buffer_size(MAX_GETPWUID_BUFFER_SIZE / 2),
            Some(MAX_GETPWUID_BUFFER_SIZE)
        );
        assert_eq!(next_getpwuid_buffer_size(MAX_GETPWUID_BUFFER_SIZE), None);
    }
}
