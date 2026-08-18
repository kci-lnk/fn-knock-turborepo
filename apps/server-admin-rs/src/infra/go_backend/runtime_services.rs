use reqwest::StatusCode;
use serde_json::Value;

use super::{
    GoBackendClient, active_ips_to_json, grpc_error, ok, parse_ssl_config, parse_waf_config,
    rpc_status_response, ssl_info_to_json, status_value, stream_active_ips_to_json,
    traffic_to_json, waf_drain_to_json, waf_status_to_json,
};
use crate::grpc_proto::{
    HostRequest, StreamRequest, WafBundleRequest, WafDrainOperation, WafDrainRequest,
};

#[allow(dead_code)]
impl GoBackendClient {
    pub async fn get_traffic_stats(&self) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.traffic.clone();
        match client.get_traffic_stats(self.request(())).await {
            Ok(response) => Ok(ok(traffic_to_json(response.into_inner()))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn get_host_active_ips(&self, host: String) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.traffic.clone();
        match client
            .get_host_active_ips(self.request(HostRequest { host }))
            .await
        {
            Ok(response) => Ok(ok(active_ips_to_json(response.into_inner()))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn get_stream_active_ips(
        &self,
        protocol: String,
        listen_port: i32,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.traffic.clone();
        match client
            .get_stream_active_ips(self.request(StreamRequest {
                protocol,
                listen_port,
            }))
            .await
        {
            Ok(response) => Ok(ok(stream_active_ips_to_json(response.into_inner()))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn get_waf_status(&self) -> anyhow::Result<Value> {
        let mut client = self.waf.clone();
        let result = match client.get_waf_status(self.request(())).await {
            Ok(response) => ok(waf_status_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("get_waf_status", result)
    }

    pub async fn set_waf_config(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.waf.clone();
        let result = match client
            .set_waf_config(self.request(parse_waf_config(config)))
            .await
        {
            Ok(response) => ok(waf_status_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_waf_config", result)
    }

    pub async fn reload_waf_rules(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.waf.clone();
        let result = match client
            .reload_waf_bundle(self.request(WafBundleRequest {
                bundle_id: String::new(),
                bundle_path: String::new(),
                has_config: true,
                config: Some(parse_waf_config(config)),
            }))
            .await
        {
            Ok(response) => ok(waf_status_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("reload_waf_rules", result)
    }

    pub async fn lease_waf_events(&self, limit: i64) -> anyhow::Result<Value> {
        self.waf_event_lease_request(
            "lease_waf_events",
            WafDrainRequest {
                limit: i32::try_from(limit).unwrap_or(i32::MAX),
                operation: WafDrainOperation::Lease as i32,
                lease_id: String::new(),
            },
        )
        .await
    }

    pub async fn acknowledge_waf_event_lease(&self, lease_id: &str) -> anyhow::Result<Value> {
        self.waf_event_lease_request(
            "acknowledge_waf_event_lease",
            WafDrainRequest {
                limit: 0,
                operation: WafDrainOperation::Acknowledge as i32,
                lease_id: lease_id.to_string(),
            },
        )
        .await
    }

    pub async fn release_waf_event_lease(&self, lease_id: &str) -> anyhow::Result<Value> {
        self.waf_event_lease_request(
            "release_waf_event_lease",
            WafDrainRequest {
                limit: 0,
                operation: WafDrainOperation::Release as i32,
                lease_id: lease_id.to_string(),
            },
        )
        .await
    }

    async fn waf_event_lease_request(
        &self,
        operation: &str,
        request: WafDrainRequest,
    ) -> anyhow::Result<Value> {
        let mut client = self.waf.clone();
        let result = match client.drain_waf_events(self.request(request)).await {
            Ok(response) => ok(waf_drain_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value(operation, result)
    }

    pub async fn get_ssl_info(&self) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.ssl.clone();
        match client.get_ssl_info(self.request(())).await {
            Ok(response) => Ok(ok(ssl_info_to_json(response.into_inner()))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn set_ssl_deployment(
        &self,
        deployment: &Value,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.ssl.clone();
        match client
            .set_ssl_deployment(self.request(parse_ssl_config(deployment)))
            .await
        {
            Ok(response) => Ok(rpc_status_response(response.into_inner())),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn clear_ssl(&self) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.ssl.clone();
        match client.clear_ssl(self.request(())).await {
            Ok(response) => Ok(rpc_status_response(response.into_inner())),
            Err(error) => Ok(grpc_error(error)),
        }
    }
}
