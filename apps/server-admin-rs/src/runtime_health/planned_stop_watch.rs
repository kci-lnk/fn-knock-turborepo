//! Wake the stop handshake on atomic request-file replacement. Polling remains
//! the correctness fallback; notifications only reduce idle file-system work.
use std::{io, path::Path};

#[cfg(target_os = "linux")]
pub(super) struct RequestWatch(tokio::io::unix::AsyncFd<std::fs::File>);

#[cfg(target_os = "linux")]
impl RequestWatch {
    pub(super) fn new(directory: &Path) -> io::Result<Self> {
        use std::{
            ffi::CString,
            os::{
                fd::{AsRawFd, FromRawFd},
                unix::ffi::OsStrExt,
            },
        };
        let path = CString::new(directory.as_os_str().as_bytes())?;
        // SAFETY: inotify_init1 has no pointer arguments. File owns the returned
        // descriptor immediately so every subsequent failure closes it.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        let mask = libc::IN_CLOSE_WRITE
            | libc::IN_MOVED_TO
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF
            | libc::IN_ONLYDIR;
        // Watch the directory, not the replaced request inode.
        let result = unsafe { libc::inotify_add_watch(file.as_raw_fd(), path.as_ptr(), mask) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(tokio::io::unix::AsyncFd::new(file)?))
    }

    pub(super) async fn changed(&self) -> io::Result<()> {
        use std::io::Read;
        let mut buffer = [0u8; 8192];
        loop {
            let mut ready = self.0.readable().await?;
            match ready.try_io(|fd| (&*fd.get_ref()).read(&mut buffer)) {
                Ok(Ok(0)) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                Ok(Ok(count)) => {
                    if request_changed(&buffer[..count])? {
                        return Ok(());
                    }
                }
                Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => continue,
                Ok(Err(error)) => return Err(error),
                Err(_) => continue,
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn request_changed(mut bytes: &[u8]) -> io::Result<bool> {
    let mut changed = false;
    let header = std::mem::size_of::<libc::inotify_event>();
    while !bytes.is_empty() {
        if bytes.len() < header {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }
        // Kernel records are length-checked, but the byte buffer need not be aligned.
        let event =
            unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<libc::inotify_event>()) };
        let size = header
            .checked_add(event.len as usize)
            .filter(|size| *size <= bytes.len())
            .ok_or(io::ErrorKind::InvalidData)?;
        if event.mask
            & (libc::IN_Q_OVERFLOW | libc::IN_IGNORED | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF)
            != 0
        {
            return Err(io::Error::other("planned-stop directory watch invalidated"));
        }
        let name = bytes[header..size]
            .split(|byte| *byte == 0)
            .next()
            .unwrap_or_default();
        changed |=
            name == b"request.json" && event.mask & (libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO) != 0;
        bytes = &bytes[size..];
    }
    Ok(changed)
}

#[cfg(not(target_os = "linux"))]
pub(super) struct RequestWatch;

#[cfg(not(target_os = "linux"))]
impl RequestWatch {
    pub(super) fn new(_: &Path) -> io::Result<Self> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
    pub(super) async fn changed(&self) -> io::Result<()> {
        std::future::pending().await
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn atomic_replacements_wake_and_unrelated_writes_do_not() {
        let directory = tempfile::tempdir().unwrap();
        let watcher = RequestWatch::new(directory.path()).unwrap();
        std::fs::write(directory.path().join("ack"), b"ack").unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), watcher.changed())
                .await
                .is_err()
        );
        for value in [b"prepare", b"cancel!", b"stopped"] {
            std::fs::write(directory.path().join("request.tmp"), value).unwrap();
            std::fs::rename(
                directory.path().join("request.tmp"),
                directory.path().join("request.json"),
            )
            .unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(1), watcher.changed())
                .await
                .unwrap()
                .unwrap();
        }
        std::fs::write(directory.path().join("request.json"), b"direct").unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), watcher.changed())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn deleted_directory_invalidates_watch() {
        let directory = tempfile::tempdir().unwrap();
        let watcher = RequestWatch::new(directory.path()).unwrap();
        std::fs::remove_dir(directory.path()).unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), watcher.changed())
                .await
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn overflow_and_truncated_records_fail_closed() {
        let mut overflow = Vec::new();
        overflow.extend_from_slice(&(-1i32).to_ne_bytes());
        overflow.extend_from_slice(&libc::IN_Q_OVERFLOW.to_ne_bytes());
        overflow.extend_from_slice(&0u32.to_ne_bytes());
        overflow.extend_from_slice(&0u32.to_ne_bytes());
        assert!(request_changed(&overflow).is_err());
        assert!(request_changed(&overflow[..3]).is_err());
    }
}
