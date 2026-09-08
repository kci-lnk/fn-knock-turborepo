use super::*;

impl GoBackendClient {
    pub async fn get_gateway_http3(&self) -> anyhow::Result<Value> {
        let response = self
            .control
            .clone()
            .get_gateway_http3_status(self.request(()))
            .await?
            .into_inner();
        Ok(http3_status_json(response))
    }
    pub async fn set_gateway_http3(&self, config: &Value) -> anyhow::Result<Value> {
        let client = self.with_timeout(Duration::from_secs(45))?;
        let response = client
            .control
            .clone()
            .set_gateway_http3_config(
                client.request(crate::grpc_proto::GatewayHttp3Config {
                    enabled: config
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    advertised_port: config
                        .get("advertised_port")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                }),
            )
            .await?
            .into_inner();
        Ok(http3_status_json(response))
    }
}
fn http3_status_json(status: crate::grpc_proto::GatewayHttp3Status) -> Value {
    let config = status.config.unwrap_or_default();
    json!({"enabled":config.enabled,"advertised_port":config.advertised_port,
        "state":status.state,"listen_addresses":status.listen_addresses,"error":status.error,
        "active_connections":status.active_connections,"handshake_failures":status.handshake_failures})
}
