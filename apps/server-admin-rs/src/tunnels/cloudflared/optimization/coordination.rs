use super::*;

pub(in crate::tunnels::cloudflared) async fn configured_optimization_hosts(
    state: &AppState,
    config: &Value,
) -> Result<Vec<String>, CloudflareApiError> {
    let settings = load_domain_settings(state).await?;
    Ok(partition_optimization_hosts(configured_hosts(config), &settings).0)
}

pub(in crate::tunnels::cloudflared) fn start_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("cloudflare-optimization-scheduler", async move {
        let mut interval = time::interval(super::super::managed::plan_wakeup_delay());
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                _ = interval.tick() => {},
                _ = task_state.tunnel.cloudflared_schedule_notify.notified() => {},
            }
            if let Err(error) = scheduled_tick(&task_state).await {
                tracing::warn!(%error, "Cloudflare optimization scheduler failed");
                let mut runtime = load_runtime(&task_state).await;
                ensure_object(&mut runtime)
                    .insert("lastError".to_string(), json!(error.to_string()));
                let _ = save_runtime(&task_state, &runtime).await;
            }
        }
    });
}

pub(in crate::tunnels::cloudflared) fn schedule_after_host_mappings_change(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("cloudflare-mapping-reconcile", async move {
        let managed = load_managed_config(&task_state).await;
        if managed.get("mode").and_then(Value::as_str) != Some("managed") {
            return;
        }
        task_state.tunnel.cloudflared_schedule_notify.notify_one();
    });
}
