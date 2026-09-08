//! Private credential ACLs shared by local secret stores.
use std::path::{Path, PathBuf};

pub(crate) fn secure_windows_path(path: &Path, directory: bool) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let icacls = PathBuf::from(system_root)
        .join("System32")
        .join("icacls.exe");
    if !icacls.is_file() {
        return Err(format!(
            "required Windows ACL tool is missing: {}",
            icacls.display()
        ));
    }
    let grants: &[&str] = if directory {
        &[
            "*S-1-5-18:F",
            "*S-1-5-18:(OI)(CI)F",
            "*S-1-5-32-544:F",
            "*S-1-5-32-544:(OI)(CI)F",
            r"NT SERVICE\FnKnock:M",
            r"NT SERVICE\FnKnock:(OI)(CI)M",
        ]
    } else {
        &["*S-1-5-18:F", "*S-1-5-32-544:F", r"NT SERVICE\FnKnock:M"]
    };
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
