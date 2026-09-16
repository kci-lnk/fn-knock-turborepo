//! Private credential ACLs shared by local secret stores.
use std::path::{Path, PathBuf};

pub(crate) fn secure_windows_path(path: &Path, directory: bool) -> Result<(), String> {
    // The Windows test suite deliberately overwrites and removes its own
    // temporary fixtures. This escape hatch is available only to debug builds
    // that explicitly opt in; release binaries always enforce the ACL below.
    if cfg!(debug_assertions)
        && std::env::var("FN_KNOCK_TEST_DISABLE_PRIVATE_ACLS").as_deref() == Ok("1")
    {
        return Ok(());
    }
    use std::process::{Command, Stdio};

    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let system_directory = PathBuf::from(system_root).join("System32");
    let icacls = system_directory.join("icacls.exe");
    if !icacls.is_file() {
        return Err(format!(
            "required Windows ACL tool is missing: {}",
            icacls.display()
        ));
    }
    // `showsid` derives the stable service SID even before the service is
    // installed. Numeric SID grants avoid account-name lookup error 1332.
    let output = Command::new(system_directory.join("sc.exe"))
        .args(["showsid", "FnKnock"])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("query FnKnock service SID: {error}"))?;
    if !output.status.success() {
        return Err(format!("sc.exe showsid failed with {}", output.status));
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let service_sid = output
        .split_whitespace()
        .find(|value| is_service_sid(value))
        .ok_or_else(|| "sc.exe did not return a valid service SID".to_string())?;
    // The service normally invokes this as its own SID. Tests and initial
    // provisioning may invoke it as a different principal, which must retain
    // access to finish the atomic write and cleanup.
    let current_sid = current_windows_sid(&system_directory)?;
    let mut grants = vec![
        "*S-1-5-18:F".to_string(),
        "*S-1-5-32-544:F".to_string(),
        format!("*{service_sid}:M"),
        format!("*{current_sid}:M"),
    ];
    if directory {
        grants.extend([
            "*S-1-5-18:(OI)(CI)F".to_string(),
            "*S-1-5-32-544:(OI)(CI)F".to_string(),
            format!("*{service_sid}:(OI)(CI)M"),
            format!("*{current_sid}:(OI)(CI)M"),
        ]);
    }
    let mut status = Command::new(&icacls)
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .args(grants)
        .args(["/L", "/Q"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    // Test fixtures and first-run installs may not have the service SID
    // registered yet. Retain access for the creating principal so the
    // operation can complete and later cleanup/rotation remains possible;
    // SYSTEM and Administrators remain the only other principals.
    if !status.success() && status.code() == Some(1332) {
        let fallback = vec![
            "*S-1-5-18:F".to_string(),
            "*S-1-5-32-544:F".to_string(),
            format!("*{current_sid}:M"),
        ];
        let mut fallback = fallback;
        if directory {
            fallback.extend([
                "*S-1-5-18:(OI)(CI)F".to_string(),
                "*S-1-5-32-544:(OI)(CI)F".to_string(),
                format!("*{current_sid}:(OI)(CI)M"),
            ]);
        }
        status = Command::new(&icacls)
            .arg(path)
            .args(["/inheritance:r", "/grant:r"])
            .args(fallback)
            .args(["/L", "/Q"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| error.to_string())?;
    }
    if !status.success() {
        return Err(format!("icacls.exe failed with {status}"));
    }
    Ok(())
}

fn current_windows_sid(system_directory: &Path) -> Result<String, String> {
    let output = std::process::Command::new(system_directory.join("whoami.exe"))
        .args(["/user", "/fo", "csv", "/nh"])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("query current Windows SID: {error}"))?;
    if !output.status.success() {
        return Err(format!("whoami.exe /user failed with {}", output.status));
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let sid = output
        .lines()
        .find_map(|line| line.rsplit(',').next())
        .map(|sid| sid.trim().trim_matches('"'))
        .filter(|sid| is_windows_sid(sid))
        .ok_or_else(|| "whoami.exe did not return a valid current-user SID".to_string())?;
    Ok(sid.to_string())
}

fn is_windows_sid(value: &str) -> bool {
    value.starts_with("S-1-")
        && value.split('-').skip(2).all(|component| {
            !component.is_empty()
                && component.bytes().all(|byte| byte.is_ascii_digit())
                && component.parse::<u32>().is_ok()
        })
}

fn is_service_sid(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("S-1-5-80-") else {
        return false;
    };
    let components: Vec<_> = suffix.split('-').collect();
    components.len() == 5
        && components.iter().all(|component| {
            !component.is_empty()
                && component.bytes().all(|byte| byte.is_ascii_digit())
                && component.parse::<u32>().is_ok()
        })
}

#[cfg(test)]
mod tests {
    use super::{is_service_sid, is_windows_sid};

    #[test]
    fn service_sid_validation_rejects_other_principals_and_acl_syntax() {
        assert!(is_service_sid("S-1-5-80-1-2-3-4-4294967295"));
        for invalid in [
            "S-1-1-0",
            "S-1-5-80-1-2-3-4",
            "S-1-5-80-1-2-3-4-4294967296",
            "S-1-5-80-1-2-3-4-5:F",
            "S-1-5-80-1-2-3-4-+5",
        ] {
            assert!(!is_service_sid(invalid), "{invalid}");
        }
    }

    #[test]
    fn current_user_sid_validation_requires_numeric_windows_sid() {
        assert!(is_windows_sid("S-1-5-21-1-2-3-1001"));
        assert!(!is_windows_sid("S-1-5-21-1-2-3-user"));
        assert!(!is_windows_sid("S-1-5-21-1-2-3-4294967296"));
    }
}
