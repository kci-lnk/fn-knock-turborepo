use std::{fs, io::ErrorKind, path::Path};

use crate::{
    admin_panel::normalize_locale_config, gateway_settings::sync_gateway_settings_on_boot,
    runtime_config::sync_runtime_config_on_boot, runtime_profile,
    ssl::sync_ssl_deployment_to_gateway, state::AppState, time_utils,
};

const CLEAN_SCRIPT_CONTENT: &str = r#"#!/bin/bash

FILTER_CHAINS=("FN-KNOCK-FW" "FN-KNOCK-SSH" "FNK_FNC_IN" "FNK-WHITELIST" "FNK-WL-MARK" "FNK-SSH-ALLOW" "FNK-SSH-BLOCK" "FNK-SSH-DEFAULT")
NAT_CHAINS=("FNK_FNC_PRE" "FNK_FNC_OUT" "FNK_FNC_WAF")
FILTER_PARENTS=("INPUT" "DOCKER-USER")
NAT_PARENTS=("PREROUTING" "OUTPUT")
FIREWALLS=("iptables" "ip6tables")
NFT_TABLES=("fnknock_ssh" "fnknock_whitelist")

remove_parent_jumps() {
    local cmd="$1"
    local table="$2"
    local parent="$3"
    local chain="$4"

    if ! "$cmd" -t "$table" -L "$parent" -n >/dev/null 2>&1; then
        return
    fi

    while IFS= read -r line; do
        [[ "$line" == "-A $parent "* ]] || continue
        [[ "$line" == *" -j $chain"* ]] || continue

        local rule_args="${line#-A $parent }"
        rule_args="${rule_args//\"/}"
        # shellcheck disable=SC2086
        if "$cmd" -t "$table" -D "$parent" $rule_args 2>/dev/null; then
            echo "Removed $table jump rule from $parent -> $chain: $rule_args"
        fi
    done < <("$cmd" -t "$table" -S "$parent" 2>/dev/null || true)

    while "$cmd" -t "$table" -D "$parent" -j "$chain" 2>/dev/null; do
        echo "Removed legacy $table jump rule from $parent -> $chain"
    done
}

cleanup_chain() {
    local cmd="$1"
    local table="$2"
    local chain="$3"
    shift 3

    local parent=""
    for parent in "$@"; do
        remove_parent_jumps "$cmd" "$table" "$parent" "$chain"
    done

    if "$cmd" -t "$table" -L "$chain" -n >/dev/null 2>&1; then
        "$cmd" -t "$table" -F "$chain"
        echo "Flushed all rules inside $table/$chain"

        "$cmd" -t "$table" -X "$chain"
        echo "Deleted custom chain $table/$chain"
    else
        echo "Chain $table/$chain does not exist in $cmd (already clean)."
    fi
}

echo "Starting firewall cleanup..."

for cmd in "${FIREWALLS[@]}"; do
    if ! command -v "$cmd" &> /dev/null; then
        echo "$cmd is not installed or not in PATH, skipping..."
        continue
    fi

    echo "--- Processing $cmd ---"

    for chain in "${FILTER_CHAINS[@]}"; do
        cleanup_chain "$cmd" filter "$chain" "${FILTER_PARENTS[@]}"
    done
    for chain in "${NAT_CHAINS[@]}"; do
        cleanup_chain "$cmd" nat "$chain" "${NAT_PARENTS[@]}"
    done
done

if command -v nft &> /dev/null; then
    for table in "${NFT_TABLES[@]}"; do
        if nft delete table inet "$table" 2>/dev/null; then
            echo "Deleted native nft interval table inet/$table"
        else
            echo "Native nft interval table inet/$table does not exist (already clean)."
        fi
    done
fi

echo "Cleanup complete!"
"#;

pub(super) fn start_boot_sync_tasks(state: AppState) -> tokio::sync::oneshot::Receiver<()> {
    let (completed_tx, completed_rx) = tokio::sync::oneshot::channel();
    let task_state = state.clone();
    state.spawn_background("boot-sync", async move {
        if let Err(error) = cleanup_legacy_auth_log_storage(&task_state).await {
            tracing::warn!(%error, "failed to cleanup legacy auth log storage on boot");
        }
        sync_runtime_config_on_boot(task_state.clone()).await;
        sync_gateway_settings_on_boot(task_state.clone()).await;
        sync_locale_config_on_boot(&task_state).await;
        if let Err(error) = sync_ssl_deployment_to_gateway(&task_state, None).await {
            tracing::warn!(%error, "failed to sync SSL deployment on boot");
        }
        if let Err(error) = init_clean_script_on_boot(&task_state) {
            tracing::warn!(%error, "failed to initialize firewall cleanup script");
        }
        let _ = completed_tx.send(());
    });
    completed_rx
}

fn init_clean_script_on_boot(state: &AppState) -> anyhow::Result<()> {
    let script_path = state.settings.data_dir.join("clean.sh");
    if !runtime_profile::host_firewall_available(state) {
        if remove_clean_script_if_present(&script_path)? {
            tracing::info!(
                path = %script_path.display(),
                "removed stale firewall cleanup script"
            );
        } else {
            tracing::info!("skipped clean.sh generation: host firewall is unavailable");
        }
        return Ok(());
    }
    fs::create_dir_all(&state.settings.data_dir)?;
    fs::write(&script_path, CLEAN_SCRIPT_CONTENT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
    }
    tracing::info!(path = %script_path.display(), "initialized firewall cleanup script");
    Ok(())
}

fn remove_clean_script_if_present(script_path: &Path) -> anyhow::Result<bool> {
    match fs::remove_file(script_path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn cleanup_legacy_auth_log_storage(state: &AppState) -> anyhow::Result<()> {
    const STATE_KEY: &str = "fn_knock:cleanup:legacy-auth-logs:v1";
    const LOCK_KEY: &str = "fn_knock:cleanup:legacy-auth-logs:v1:lock";
    const INDEX_KEY: &str = "fn_knock:auth_logs:index";
    const DATA_PREFIX: &str = "fn_knock:auth_log_data:";
    const REF_PREFIX: &str = "fn_knock:ip_location:refs:";
    const LEGACY_REF_PREFIX: &str = "auth-log|";

    if state
        .storage
        .store
        .get_string_value(STATE_KEY)
        .await?
        .as_deref()
        == Some("done")
    {
        return Ok(());
    }
    if !state
        .storage
        .store
        .set_key_if_not_exists_with_ttl(LOCK_KEY, &time_utils::now_ms().to_string(), 3600)
        .await?
    {
        return Ok(());
    }

    let cleanup_result = async {
        state
            .storage
            .store
            .set_string_value_with_optional_ttl(STATE_KEY, "running", Some(3600))
            .await?;
        let data_keys = state.storage.store.scan_keys(DATA_PREFIX, 200).await?;
        for chunk in data_keys.chunks(200) {
            state.storage.store.delete_keys(chunk).await?;
        }
        state.storage.store.delete_key(INDEX_KEY).await?;

        let ref_keys = state.storage.store.scan_keys(REF_PREFIX, 200).await?;
        for key in ref_keys {
            let members = state.storage.store.smembers_strings(&key).await?;
            let legacy_members = members
                .into_iter()
                .filter(|member| member.starts_with(LEGACY_REF_PREFIX))
                .collect::<Vec<_>>();
            state
                .storage
                .store
                .srem_string_members(&key, &legacy_members)
                .await?;
        }
        state
            .storage
            .store
            .set_string_value(STATE_KEY, "done")
            .await
    }
    .await;

    let _ = state.storage.store.delete_key(LOCK_KEY).await;
    cleanup_result.map_err(Into::into)
}

async fn sync_locale_config_on_boot(state: &AppState) {
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for locale boot sync");
            return;
        }
    };
    let locale = normalize_locale_config(config.get("locale").unwrap_or(&serde_json::Value::Null));
    match state.gateway.client.set_locale_config(&locale).await {
        Ok((status, value)) if status == reqwest::StatusCode::NOT_FOUND => {
            tracing::debug!(?value, "gateway locale sync endpoint is unavailable");
        }
        Ok((status, value)) => {
            if !status.is_success()
                || value.get("success").and_then(serde_json::Value::as_bool) == Some(false)
            {
                tracing::warn!(%status, response = %value, "failed to sync locale config on boot");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to sync locale config on boot");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firewall_cleanup_script_covers_all_fn_connect_waf_chains_and_parents() {
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK_FNC_WAF\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK_FNC_OUT\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK_FNC_PRE\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK_FNC_IN\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK-WHITELIST\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK-WL-MARK\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK-SSH-ALLOW\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK-SSH-BLOCK\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"FNK-SSH-DEFAULT\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"OUTPUT\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"PREROUTING\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"INPUT\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("-t \"$table\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"fnknock_ssh\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("\"fnknock_whitelist\""));
        assert!(CLEAN_SCRIPT_CONTENT.contains("nft delete table inet \"$table\""));
    }

    #[test]
    fn stale_firewall_cleanup_script_can_be_removed_without_execution() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let script_path = directory.path().join("clean.sh");
        fs::write(&script_path, "#!/bin/sh\nexit 99\n").expect("legacy clean.sh");

        assert!(remove_clean_script_if_present(&script_path).expect("remove clean.sh"));
        assert!(!script_path.exists());
        assert!(!remove_clean_script_if_present(&script_path).expect("ignore missing clean.sh"));
    }
}
