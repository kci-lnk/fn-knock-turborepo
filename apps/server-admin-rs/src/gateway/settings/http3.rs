use super::*;

pub(super) fn http3_config(value: &Value) -> Result<Value, String> {
    let enabled = value
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| "HTTP/3 enabled must be a boolean".to_string())?;
    let port = match value.get("advertised_port") {
        None | Some(Value::Null) => 0,
        Some(port) => port
            .as_u64()
            .filter(|port| *port <= 65535)
            .ok_or_else(|| "HTTP/3 advertised_port must be between 0 and 65535".to_string())?,
    };
    Ok(json!({"enabled": enabled, "advertised_port": port}))
}

pub(super) async fn sync_http3(state: &AppState, config: &Value) -> Result<(), String> {
    let config = http3_config(
        config
            .get("gateway_http3")
            .unwrap_or(&json!({"enabled":false})),
    )?;
    state
        .gateway
        .client
        .set_gateway_http3(&config)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub(super) async fn update_http3(state: &AppState, config: Value) -> Result<Value, String> {
    let normalized = http3_config(&config)?;
    proxy_config::with_host_mappings_runtime_transaction(state, move |state| async move {
        let previous = state
            .storage
            .store
            .get_config()
            .await
            .map_err(|e| e.to_string())?;
        state
            .gateway
            .client
            .set_gateway_http3(&normalized)
            .await
            .map_err(|e| e.to_string())?;
        let mut next = previous.clone();
        ensure_object(&mut next).insert("gateway_http3".to_string(), normalized);
        if let Err(error) = state.storage.store.save_config(&next).await {
            let rollback = sync_http3(&state, &previous).await;
            return Err(match rollback {
                Ok(()) => error.to_string(),
                Err(rollback) => format!("{error}; HTTP/3 rollback failed: {rollback}"),
            });
        }
        Ok(())
    })
    .await?;
    state
        .gateway
        .client
        .get_gateway_http3()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_http3_port_and_enabled() {
        assert_eq!(
            http3_config(&json!({"enabled":false})).unwrap(),
            json!({"enabled":false,"advertised_port":0})
        );
        for value in [
            json!({"enabled":"true"}),
            json!({"enabled":true,"advertised_port":65536}),
            json!({"enabled":true,"advertised_port":-1}),
            json!({"enabled":true,"advertised_port":443.5}),
        ] {
            assert!(http3_config(&value).is_err());
        }
        assert!(http3_config(&json!({"enabled":true,"advertised_port":443})).is_ok());
    }
}
