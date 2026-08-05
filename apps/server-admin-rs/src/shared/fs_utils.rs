use std::{io, path::Path};

use tokio::io::AsyncReadExt;

pub(crate) async fn read_file_limited(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let file = tokio::fs::File::open(path).await?;
    read_open_file_limited(file, limit).await.map_err(|error| {
        if error.kind() == io::ErrorKind::InvalidData {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} exceeds the {limit}-byte limit", path.display()),
            )
        } else {
            error
        }
    })
}

pub(crate) async fn read_open_file_limited(
    file: tokio::fs::File,
    limit: usize,
) -> io::Result<Vec<u8>> {
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut content = Vec::with_capacity(limit.min(64 * 1024));
    file.take(read_limit).read_to_end(&mut content).await?;
    if content.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("file exceeds the {limit}-byte limit"),
        ));
    }
    Ok(content)
}

#[cfg(not(windows))]
pub(crate) fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    std::fs::rename(temporary, destination)
}

#[cfg(windows)]
pub(crate) fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are valid, NUL-terminated UTF-16 paths and remain
    // alive for the duration of this same-volume atomic replacement call.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
pub(crate) fn chmod_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
pub(crate) fn chmod_executable(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn chmod_executable_sets_expected_unix_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tool");
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();

        chmod_executable(&path);

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[tokio::test]
    async fn limited_file_read_accepts_the_limit_and_rejects_the_next_byte() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bounded");
        std::fs::write(&path, b"1234").unwrap();
        assert_eq!(read_file_limited(&path, 4).await.unwrap(), b"1234");

        std::fs::write(&path, b"12345").unwrap();
        let error = read_file_limited(&path, 4).await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}
