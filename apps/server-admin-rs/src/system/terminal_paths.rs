use std::{
    env,
    path::{Component, Path, PathBuf},
};

pub(crate) const DEFAULT_TERMINAL_CWD: &str = "~";

pub(crate) fn normalize_terminal_default_cwd(value: Option<&str>) -> String {
    let runtime_dirs = terminal_runtime_dir_candidates();
    normalize_terminal_default_cwd_with_candidates(value, &runtime_dirs)
}

pub(crate) fn is_terminal_runtime_cwd(value: &str) -> bool {
    let runtime_dirs = terminal_runtime_dir_candidates();
    is_terminal_runtime_cwd_with_candidates(value, &runtime_dirs)
}

fn normalize_terminal_default_cwd_with_candidates(
    value: Option<&str>,
    runtime_dirs: &[PathBuf],
) -> String {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_TERMINAL_CWD);
    if is_terminal_runtime_cwd_with_candidates(value, runtime_dirs) {
        DEFAULT_TERMINAL_CWD.to_string()
    } else {
        value.to_string()
    }
}

fn is_terminal_runtime_cwd_with_candidates(value: &str, runtime_dirs: &[PathBuf]) -> bool {
    let Some(candidate) = normalize_absolute_path(Path::new(value.trim())) else {
        return false;
    };
    runtime_dirs
        .iter()
        .any(|runtime_dir| candidate == *runtime_dir || candidate.starts_with(runtime_dir))
}

fn terminal_runtime_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(current_dir) = env::current_dir() {
        push_runtime_dir_candidate(&mut candidates, current_dir);
    }

    for key in [
        "TRIM_APPDEST",
        "TRIM_PKGVAR",
        "FN_KNOCK_APP_HOME",
        "FN_KNOCK_DATA_DIR",
        "FN_KNOCK_GATEWAY_CONFIG_DIR",
        "GATEWAY_CONFIG_DIR",
    ] {
        if let Ok(value) = env::var(key) {
            push_runtime_dir_candidate(&mut candidates, PathBuf::from(value.trim()));
        }
    }

    if let Ok(current_exe) = env::current_exe()
        && let Some(exe_dir) = current_exe.parent()
    {
        push_runtime_dir_candidate(&mut candidates, exe_dir.to_path_buf());
        if exe_dir
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name == "bin" || name == "server")
            && let Some(app_dir) = exe_dir.parent()
        {
            push_runtime_dir_candidate(&mut candidates, app_dir.to_path_buf());
        }
    }

    for path in [
        "/opt/fn-knock",
        "/usr/lib/fn-knock",
        "/usr/local/etc/fn-knock",
        "/etc/fn-knock/gateway",
        "/var/lib/fn-knock",
        "/tmp/fn-knock",
    ] {
        push_runtime_dir_candidate(&mut candidates, PathBuf::from(path));
    }

    candidates
}

fn push_runtime_dir_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    let Some(path) = normalize_absolute_path(&path) else {
        return;
    };
    if path == Path::new("/") || candidates.iter().any(|candidate| candidate == &path) {
        return;
    }
    candidates.push(path);
}

fn normalize_absolute_path(path: &Path) -> Option<PathBuf> {
    // Windows treats a separator-rooted legacy Unix path (for example
    // `/opt/fn-knock`) as rooted but not fully absolute because it has no
    // drive prefix. Keep the check lexical so old Linux runtime values can be
    // recognized and migrated when the same config is opened on Windows.
    if !path.has_root() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_default_cwd_falls_back_to_tilde_for_empty_values() {
        assert_eq!(
            normalize_terminal_default_cwd_with_candidates(None, &[]),
            DEFAULT_TERMINAL_CWD
        );
        assert_eq!(
            normalize_terminal_default_cwd_with_candidates(Some("   "), &[]),
            DEFAULT_TERMINAL_CWD
        );
    }

    #[test]
    fn terminal_default_cwd_preserves_user_paths() {
        let runtime_dirs = vec![PathBuf::from("/opt/fn-knock")];
        assert_eq!(
            normalize_terminal_default_cwd_with_candidates(Some("/root"), &runtime_dirs),
            "/root"
        );
        assert_eq!(
            normalize_terminal_default_cwd_with_candidates(Some("~/work"), &runtime_dirs),
            "~/work"
        );
    }

    #[test]
    fn terminal_default_cwd_replaces_runtime_directories_with_tilde() {
        let runtime_dirs = vec![
            PathBuf::from("/opt/fn-knock"),
            PathBuf::from("/usr/local/etc/fn-knock"),
        ];
        assert_eq!(
            normalize_terminal_default_cwd_with_candidates(Some("/opt/fn-knock/"), &runtime_dirs),
            DEFAULT_TERMINAL_CWD
        );
        assert_eq!(
            normalize_terminal_default_cwd_with_candidates(
                Some("/usr/local/etc/fn-knock/./logs/.."),
                &runtime_dirs
            ),
            DEFAULT_TERMINAL_CWD
        );
        assert_eq!(
            normalize_terminal_default_cwd_with_candidates(
                Some("/usr/local/etc/fn-knock/logs"),
                &runtime_dirs
            ),
            DEFAULT_TERMINAL_CWD
        );
    }
}
