use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::OnceLock,
};

use regex::Regex;
use serde_json::json;

const MAX_TAIL_BYTES: u64 = 256 * 1024;
const FILE_NAMES: [&str; 4] = [
    "info.log",
    "info.log.1",
    "gateway-runtime.log",
    "gateway-runtime.log.1",
];

// Read only fixed application-owned files. Do not run journalctl/dmesg or
// recursively collect the data directory from an authenticated HTTP request.
pub(super) fn append(data_dir: &Path, files: &mut Vec<(String, Vec<u8>)>) -> anyhow::Result<()> {
    let mut collection = Vec::new();
    for name in FILE_NAMES {
        match read_tail(&data_dir.join(name)) {
            Ok((raw, truncated)) => {
                let data = redact_tail(&raw).into_bytes();
                let truncated = truncated || data.len() > MAX_TAIL_BYTES as usize;
                let data = super::tail_at_line_boundary(data, MAX_TAIL_BYTES as usize);
                collection.push(json!({
                    "file": name,
                    "status": if data.is_empty() { "empty" } else { "included" },
                    "tail_truncated": truncated,
                    "exported_bytes": data.len(),
                }));
                files.push((format!("logs/{name}"), data));
            }
            Err(error) => collection.push(json!({
                "file": name,
                "status": if error.kind() == std::io::ErrorKind::NotFound {
                    "missing"
                } else {
                    "unreadable"
                },
                // Error messages may contain private filesystem paths.
                "error_kind": format!("{:?}", error.kind()),
            })),
        }
    }
    files.push((
        "logs/startup-collection.json".to_string(),
        serde_json::to_vec_pretty(&json!({
            "max_tail_bytes_per_file": MAX_TAIL_BYTES,
            "time_filter": false,
            "files": collection,
        }))?,
    ));
    Ok(())
}

fn read_tail(path: &Path) -> std::io::Result<(Vec<u8>, bool)> {
    // Reject links and non-files before opening (in particular FIFOs).
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "expected regular log file",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Enforce this on open as well: a file can be replaced after lstat.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "expected regular log file",
        ));
    }
    let length = metadata.len();
    let offset = length.saturating_sub(MAX_TAIL_BYTES);
    let partial_line = if offset > 0 {
        file.seek(SeekFrom::Start(offset - 1))?;
        let mut previous = [0];
        file.read_exact(&mut previous)?;
        previous[0] != b'\n'
    } else {
        false
    };
    let mut raw = Vec::new();
    file.take(length - offset).read_to_end(&mut raw)?;
    if partial_line {
        // Never emit a suffix of a line: it may have lost a secret's field name
        // or begin in the middle of a UTF-8 character.
        let start = raw.iter().position(|b| *b == b'\n').map(|i| i + 1);
        raw = start.map(|i| raw[i..].to_vec()).unwrap_or_default();
    }
    Ok((raw, offset > 0))
}

fn redact_tail(raw: &[u8]) -> String {
    let mut output = String::new();
    let text = String::from_utf8_lossy(raw);
    let text = ansi_regex().replace_all(&text, "");
    // If the first PEM boundary is END, this tail starts inside that block.
    // Include short final base64 lines and timestamp-prefixed PEM contents;
    // a long-base64 heuristic alone cannot recognize those fragments.
    let mut in_pem = text
        .lines()
        .find_map(|line| {
            let upper = line.to_ascii_uppercase();
            if upper.contains("-----BEGIN ") {
                Some(false)
            } else if upper.contains("-----END ") {
                Some(true)
            } else {
                None
            }
        })
        .unwrap_or(false);
    for line in text.lines() {
        let upper = line.to_ascii_uppercase();
        let begins_pem = upper.contains("-----BEGIN ");
        let ends_pem = upper.contains("-----END ");
        let trimmed = line.trim();
        // A bounded tail may start inside a PEM block, before its BEGIN line.
        let encoded_material = trimmed.len() >= 32
            && trimmed
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b));
        let redacted = if in_pem
            || begins_pem
            || ends_pem
            || encoded_material
            || sensitive_assignment().is_match(line)
        {
            "[redacted]".to_string()
        } else {
            // Some existing redaction branches (such as a bare path's file
            // name) do not truncate. Enforce the text-log contract here too.
            super::truncate_string(super::redact_string(line), 512)
        };
        if begins_pem {
            in_pem = true;
        }
        if ends_pem {
            in_pem = false;
        }
        output.push_str(&redacted);
        output.push('\n');
    }
    output
}

fn ansi_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").expect("valid ANSI CSI regex"))
}

fn sensitive_assignment() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?i)[a-z0-9_-]*(?:password|passwd|secret|token|cookie|authorization|private[_-]?key|certificate|session)[a-z0-9_-]*["']?\s*[:=]|\b(?:TRIM_[A-Z0-9_]+|PATH|ADMIN_VIEW_PORT|BACKEND_PORT|AUTH_PORT|GO_BACKEND_PORT|GO_REPROXY_PORT|FN_KNOCK_ROOT_SHARE_DIR|FN_KNOCK_CERT_SHARE_DIR)="#,
        )
        .expect("valid startup log redaction regex")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_tails_preserve_failure_evidence_and_redact_unstructured_secrets() {
        let raw = b"2026-10-26 process exited early (pid=42)\nError: Address already in use (os error 98)\n[debug] wait_result: exited with code 137\n{\"client_secret\": \"json-canary\"}\nPASSWORD = spaced-canary\nTRIM_PRIVATE=environment-canary\n-----BEGIN RSA PRIVATE KEY-----\npem-canary\n-----END RSA PRIVATE KEY-----\nat /Users/example/project/main.rs:42\nconnect http://user:pass@example.com/private\n";
        let result = redact_tail(raw);
        assert!(result.contains("process exited early (pid=42)"));
        assert!(result.contains("Address already in use (os error 98)"));
        assert!(result.contains("exited with code 137"));
        assert!(result.contains("main.rs:42"));
        for secret in ["canary", "/Users/example", "user:pass", "example.com"] {
            assert!(!result.contains(secret), "leaked {secret}: {result}");
        }
        assert!(!redact_tail(b"YWJjZGVmYWJjZGVmYWJjZGVmYWJjZGVm\n").contains("YWJj"));
        let long_path = format!("/private/{}\n", "界".repeat(400));
        assert!(redact_tail(long_path.as_bytes()).trim_end().len() <= 512);
    }

    #[test]
    fn oversized_tail_discards_partial_lines_and_keeps_latest_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("info.log");
        let mut content = b"token=".to_vec();
        content.extend(vec![b'x'; MAX_TAIL_BYTES as usize + 100]);
        content.extend_from_slice(b"\nlatest failure: signal 9\ninvalid byte: \xff\n");
        std::fs::write(&path, content).unwrap();
        let (raw, truncated) = read_tail(&path).unwrap();
        assert!(truncated);
        assert!(raw.starts_with(b"latest failure: signal 9\n"));
        let result = redact_tail(&raw);
        assert!(result.contains("latest failure: signal 9"));
        assert!(!result.contains("xxx"));
        assert!(result.len() <= MAX_TAIL_BYTES as usize);
    }

    #[test]
    fn aligned_tail_preserves_first_complete_line() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("info.log");
        let mut tail = b"first complete failure\n".to_vec();
        tail.resize(MAX_TAIL_BYTES as usize, b'\n');
        let mut content = b"old line\n".to_vec();
        content.extend_from_slice(&tail);
        std::fs::write(&path, content).unwrap();
        let (raw, truncated) = read_tail(&path).unwrap();
        assert!(truncated);
        assert_eq!(raw, tail);
    }

    #[test]
    fn truncated_pem_and_colored_credentials_are_redacted() {
        let raw = b"2026-10-26 short-pem-canary\nYWJjZA==\n-----END PRIVATE KEY-----\npassword\x1b[0m = color-canary\n\x1b[31mError: Address already in use\x1b[0m\n";
        let result = redact_tail(raw);
        assert!(!result.contains("canary"));
        assert!(!result.contains("YWJjZA"));
        assert!(!result.contains('\x1b'));
        assert!(result.contains("Error: Address already in use"));
    }

    #[cfg(unix)]
    #[test]
    fn startup_log_symlinks_and_fifos_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let private = directory.path().join("private");
        std::fs::write(&private, "private-canary").unwrap();
        let log = directory.path().join("info.log");
        std::os::unix::fs::symlink(&private, &log).unwrap();
        assert_eq!(
            read_tail(&log).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        std::fs::remove_file(&log).unwrap();
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::CString::new(log.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert_eq!(
            read_tail(&log).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn collection_reports_missing_and_unreadable_files_without_failing_export() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("info.log")).unwrap();
        let mut files = Vec::new();
        append(directory.path(), &mut files).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&files[0].1).unwrap();
        assert_eq!(report["files"][0]["status"], "unreadable");
        assert_eq!(report["files"][1]["status"], "missing");
    }

    #[test]
    fn collection_caps_expanded_redactions_and_records_truncation() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("info.log"), "token=\n".repeat(35_000)).unwrap();
        let mut files = Vec::new();
        append(directory.path(), &mut files).unwrap();
        assert_eq!(files[0].0, "logs/info.log");
        assert!(files[0].1.len() <= MAX_TAIL_BYTES as usize);
        assert!(
            String::from_utf8(files[0].1.clone())
                .unwrap()
                .starts_with("[redacted]\n")
        );
        let report: serde_json::Value = serde_json::from_slice(&files[1].1).unwrap();
        assert_eq!(report["files"][0]["tail_truncated"], true);
    }
}
