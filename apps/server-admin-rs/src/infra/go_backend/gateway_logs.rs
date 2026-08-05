use reqwest::StatusCode;
use serde_json::{Value, json};

use super::{
    GoBackendClient, grpc_error, log_analytics_to_json, log_dates_to_json, log_delete_to_json,
    log_query_to_json, logging_to_json, ok, parse_logging, status_value,
};
use crate::grpc_proto::{GatewayLogAnalyticsQuery, GatewayLogQuery, StringValue};

#[allow(dead_code)]
impl GoBackendClient {
    pub async fn set_gateway_logging_config(&self, config: &Value) -> anyhow::Result<Value> {
        status_value(
            "set_gateway_logging_config",
            self.set_gateway_logging_config_status(config).await?,
        )
    }

    pub async fn set_gateway_logging_config_status(
        &self,
        config: &Value,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.logs.clone();
        match client
            .set_logging_config(self.request(parse_logging(config)))
            .await
        {
            Ok(response) => Ok(ok(logging_to_json(response.into_inner()))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn get_logging_config(&self) -> anyhow::Result<Value> {
        let mut client = self.logs.clone();
        let result = match client.get_logging_config(self.request(())).await {
            Ok(response) => ok(logging_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("get_logging_config", result)
    }

    pub async fn get_logging_directory(&self) -> anyhow::Result<Value> {
        let mut client = self.logs.clone();
        let result = match client.get_logging_directory(self.request(())).await {
            Ok(response) => ok(json!({ "logs_dir": response.into_inner().value })),
            Err(error) => grpc_error(error),
        };
        status_value("get_logging_directory", result)
    }

    pub async fn get_log_dates(&self) -> anyhow::Result<Value> {
        let mut client = self.logs.clone();
        let result = match client.get_log_dates(self.request(())).await {
            Ok(response) => ok(log_dates_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("get_log_dates", result)
    }

    pub async fn query_log_entries(&self, query: GatewayLogQuery) -> anyhow::Result<Value> {
        let mut client = self.logs.clone();
        let result = match client.query_log_entries(self.request(query)).await {
            Ok(response) => ok(log_query_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("query_log_entries", result)
    }

    pub async fn analyze_log_entries(
        &self,
        query: GatewayLogAnalyticsQuery,
    ) -> anyhow::Result<Value> {
        let mut client = self.logs.clone();
        let result = match client.analyze_log_entries(self.request(query)).await {
            Ok(response) => ok(log_analytics_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("analyze_log_entries", result)
    }

    pub async fn delete_log_date(&self, date: &str) -> anyhow::Result<Value> {
        let mut client = self.logs.clone();
        let result = match client
            .delete_log_date(self.request(StringValue {
                value: date.to_string(),
            }))
            .await
        {
            Ok(response) => ok(log_delete_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("delete_log_date", result)
    }
}
