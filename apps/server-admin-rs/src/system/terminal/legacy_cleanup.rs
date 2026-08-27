use serde_json::Value;
use tokio::sync::Mutex;

use crate::state::AppState;

const SESSION_INDEX_KEY: &str = "fn_knock:terminal:session:index";
const SESSION_DATA_PREFIX: &str = "fn_knock:terminal:session:data:";
const SESSION_ATTACHMENTS_PREFIX: &str = "fn_knock:terminal:session:attachments:";
const ATTACHMENT_DATA_PREFIX: &str = "fn_knock:terminal:attachment:data:";
const CLEANUP_MARKER_KEY: &str = "fn_knock:terminal:migration:ssh-v1-legacy-cleanup";
static CLEANUP_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn cleanup(state: &AppState) -> anyhow::Result<()> {
    let _guard = CLEANUP_LOCK.lock().await;
    let store = &state.storage.store;
    if store.get_string_value(CLEANUP_MARKER_KEY).await?.as_deref() == Some("done") {
        return Ok(());
    }
    let mut keys = vec![SESSION_INDEX_KEY.to_string()];
    let mut tmux_sessions = Vec::new();
    for data_key in store.scan_keys(SESSION_DATA_PREFIX, 200).await? {
        if let Some(raw) = store.get_string_value(&data_key).await?
            && let Ok(value) = serde_json::from_str::<Value>(&raw)
            && let Some(name) = value.get("backend_session_name").and_then(Value::as_str)
            && valid_legacy_tmux_name(name)
        {
            tmux_sessions.push(name.to_string());
        }
        keys.push(data_key);
    }
    keys.extend(store.scan_keys(SESSION_ATTACHMENTS_PREFIX, 200).await?);
    keys.extend(store.scan_keys(ATTACHMENT_DATA_PREFIX, 200).await?);
    tmux_sessions.sort();
    tmux_sessions.dedup();
    #[cfg(unix)]
    for name in tmux_sessions {
        kill_legacy_tmux_session(&name).await?;
    }
    store.delete_keys(&keys).await?;

    let stream_dir = state.settings.data_dir.join("terminal-streams");
    match tokio::fs::remove_dir_all(&stream_dir).await {
        Ok(()) => {
            tracing::info!(path = %stream_dir.display(), "removed legacy terminal stream directory")
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    store
        .set_string_value_with_optional_ttl(CLEANUP_MARKER_KEY, "done", None)
        .await?;
    Ok(())
}

fn valid_legacy_tmux_name(value: &str) -> bool {
    value.len() == 20
        && value.starts_with("fnk_")
        && value[4..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(unix)]
async fn kill_legacy_tmux_session(name: &str) -> anyhow::Result<()> {
    for executable in ["/usr/bin/tmux", "tmux"] {
        let Some(has_session) = run_tmux(executable, &["has-session", "-t", name]).await? else {
            continue;
        };
        if !has_session.status.success() {
            if tmux_session_absent(&has_session) {
                return Ok(());
            }
            anyhow::bail!(
                "tmux failed to inspect legacy fn-knock session {name} with {}",
                has_session.status
            );
        }
        let Some(killed) = run_tmux(executable, &["kill-session", "-t", name]).await? else {
            continue;
        };
        if killed.status.success() || tmux_session_absent(&killed) {
            return Ok(());
        }
        anyhow::bail!(
            "tmux failed to stop legacy fn-knock session {name} with {}",
            killed.status
        );
    }
    // If tmux is no longer installed there cannot be a reachable tmux server
    // to terminate. The strictly-scoped legacy records can still be removed.
    Ok(())
}

#[cfg(unix)]
async fn run_tmux(
    executable: &str,
    arguments: &[&str],
) -> anyhow::Result<Option<std::process::Output>> {
    let mut command = tokio::process::Command::new(executable);
    command.args(arguments).kill_on_drop(true);
    match tokio::time::timeout(std::time::Duration::from_secs(5), command.output()).await {
        Err(_) => anyhow::bail!("timed out running legacy terminal tmux cleanup"),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Ok(Err(error)) => Err(error.into()),
        Ok(Ok(output)) => Ok(Some(output)),
    }
}

#[cfg(unix)]
fn tmux_session_absent(output: &std::process::Output) -> bool {
    tmux_session_absent_message(&output.stderr)
}

#[cfg(unix)]
fn tmux_session_absent_message(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    stderr.contains("can't find session")
        || stderr.contains("no server running")
        || stderr.contains("failed to connect to server")
        || stderr.contains("error connecting to")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_owned_legacy_tmux_names() {
        assert!(valid_legacy_tmux_name("fnk_0123456789abcdef"));
        assert!(!valid_legacy_tmux_name("fnk_0123456789abcdeF"));
        assert!(!valid_legacy_tmux_name("fnk_0123456789abcdef0"));
        assert!(!valid_legacy_tmux_name("user_0123456789abcde"));
        assert!(!valid_legacy_tmux_name("fnk_;rm____________"));
    }

    #[cfg(unix)]
    #[test]
    fn recognizes_only_idempotent_tmux_absence_errors() {
        for message in [
            "can't find session: fnk_0123456789abcdef",
            "no server running on /tmp/tmux-1000/default",
            "failed to connect to server: Connection refused",
            "error connecting to /tmp/tmux-1000/default (No such file or directory)",
        ] {
            assert!(tmux_session_absent_message(message.as_bytes()), "{message}");
        }
        assert!(!tmux_session_absent_message(
            b"open terminal failed: permission denied"
        ));
    }
}
