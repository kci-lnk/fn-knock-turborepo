//! Private credential ACLs shared by local secret stores.
use std::path::{Path, PathBuf};

pub(crate) fn secure_windows_path(path: &Path, directory: bool) -> Result<(), String> {
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
    let mut grants = vec![
        "*S-1-5-18:F".to_string(),
        "*S-1-5-32-544:F".to_string(),
        format!("*{service_sid}:M"),
    ];
    if directory {
        grants.extend([
            "*S-1-5-18:(OI)(CI)F".to_string(),
            "*S-1-5-32-544:(OI)(CI)F".to_string(),
            format!("*{service_sid}:(OI)(CI)M"),
        ]);
    }
    let status = Command::new(icacls)
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .args(grants)
        .args(["/L", "/Q"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("icacls.exe failed with {status}"));
    }
    Ok(())
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
    use super::is_service_sid;

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
}
