use std::fs;

use crate::{
    admin_panel::normalize_locale_config, gateway_settings::sync_gateway_settings_on_boot,
    runtime_config::sync_runtime_config_on_boot, runtime_profile,
    ssl::sync_ssl_deployment_to_gateway, state::AppState, time_utils,
};

const CLEAN_SCRIPT_CONTENT: &str = r#"#!/bin/bash

CHAINS=("FN-KNOCK-FW" "FN-KNOCK-SSH")
PARENTS=("INPUT" "DOCKER-USER")
TABLES=("iptables" "ip6tables")

remove_parent_jumps() {
    local cmd="$1"
    local parent="$2"
    local chain="$3"

    if ! "$cmd" -L "$parent" -n >/dev/null 2>&1; then
        return
    fi

    while IFS= read -r line; do
        [[ "$line" == "-A $parent "* ]] || continue
        [[ "$line" == *" -j $chain"* ]] || continue

        local rule_args="${line#-A $parent }"
        # shellcheck disable=SC2086
        if "$cmd" -D "$parent" $rule_args 2>/dev/null; then
            echo "Removed jump rule from $parent -> $chain: $rule_args"
        fi
    done < <("$cmd" -S "$parent" 2>/dev/null || true)

    while "$cmd" -D "$parent" -j "$chain" 2>/dev/null; do
        echo "Removed legacy jump rule from $parent -> $chain"
    done
}

echo "Starting firewall cleanup for chains: ${CHAINS[*]}..."

for cmd in "${TABLES[@]}"; do
    if ! command -v "$cmd" &> /dev/null; then
        echo "$cmd is not installed or not in PATH, skipping..."
        continue
    fi

    echo "--- Processing $cmd ---"

    for chain in "${CHAINS[@]}"; do
        for parent in "${PARENTS[@]}"; do
            remove_parent_jumps "$cmd" "$parent" "$chain"
        done

        if "$cmd" -L "$chain" -n >/dev/null 2>&1; then
            "$cmd" -F "$chain"
            echo "Flushed all rules inside $chain"

            "$cmd" -X "$chain"
            echo "Deleted custom chain $chain"
        else
            echo "Chain $chain does not exist in $cmd (already clean)."
        fi
    done
done

echo "Cleanup complete!"
"#;

pub(super) fn start_boot_sync_tasks(state: AppState) {
    tokio::spawn(async move {
        if let Err(error) = cleanup_legacy_auth_log_storage(&state).await {
            tracing::warn!(%error, "failed to cleanup legacy auth log storage on boot");
        }
        sync_runtime_config_on_boot(state.clone()).await;
        sync_gateway_settings_on_boot(state.clone()).await;
        sync_locale_config_on_boot(&state).await;
        if let Err(error) = sync_ssl_deployment_to_gateway(&state, None).await {
            tracing::warn!(%error, "failed to sync SSL deployment on boot");
        }
        if let Err(error) = init_clean_script_on_boot(&state) {
            tracing::warn!(%error, "failed to initialize firewall cleanup script");
        }
    });
}

fn init_clean_script_on_boot(state: &AppState) -> anyhow::Result<()> {
    if !runtime_profile::host_firewall_available(state) {
        tracing::info!("skipped clean.sh generation: host firewall is unavailable");
        return Ok(());
    }
    fs::create_dir_all(&state.settings.data_dir)?;
    let script_path = state.settings.data_dir.join("clean.sh");
    fs::write(&script_path, CLEAN_SCRIPT_CONTENT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
    }
    tracing::info!(path = %script_path.display(), "initialized firewall cleanup script");
    Ok(())
}

pub(crate) async fn cleanup_legacy_auth_log_storage(state: &AppState) -> anyhow::Result<()> {
    const STATE_KEY: &str = "fn_knock:cleanup:legacy-auth-logs:v1";
    const LOCK_KEY: &str = "fn_knock:cleanup:legacy-auth-logs:v1:lock";
    const INDEX_KEY: &str = "fn_knock:auth_logs:index";
    const DATA_PREFIX: &str = "fn_knock:auth_log_data:";
    const REF_PREFIX: &str = "fn_knock:ip_location:refs:";
    const LEGACY_REF_PREFIX: &str = "auth-log|";

    if state.redis.get_string_value(STATE_KEY).await?.as_deref() == Some("done") {
        return Ok(());
    }
    if !state
        .redis
        .set_key_if_not_exists_with_ttl(LOCK_KEY, &time_utils::now_ms().to_string(), 3600)
        .await?
    {
        return Ok(());
    }

    let cleanup_result = async {
        state
            .redis
            .set_string_value_with_optional_ttl(STATE_KEY, "running", Some(3600))
            .await?;
        let data_keys = state.redis.scan_keys(DATA_PREFIX, 200).await?;
        for chunk in data_keys.chunks(200) {
            state.redis.delete_keys(chunk).await?;
        }
        state.redis.delete_key(INDEX_KEY).await?;

        let ref_keys = state.redis.scan_keys(REF_PREFIX, 200).await?;
        for key in ref_keys {
            let members = state.redis.smembers_strings(&key).await?;
            let legacy_members = members
                .into_iter()
                .filter(|member| member.starts_with(LEGACY_REF_PREFIX))
                .collect::<Vec<_>>();
            state
                .redis
                .srem_string_members(&key, &legacy_members)
                .await?;
        }
        state.redis.set_string_value(STATE_KEY, "done").await
    }
    .await;

    let _ = state.redis.delete_key(LOCK_KEY).await;
    cleanup_result.map_err(Into::into)
}

async fn sync_locale_config_on_boot(state: &AppState) {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for locale boot sync");
            return;
        }
    };
    let locale = normalize_locale_config(config.get("locale").unwrap_or(&serde_json::Value::Null));
    match state.go_backend.set_locale_config(&locale).await {
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
