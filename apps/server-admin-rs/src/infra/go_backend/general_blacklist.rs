use serde_json::Value;

use super::{
    GoBackendClient, general_blacklist_list_to_json, general_blacklist_mutation_to_json,
    general_blacklist_status_to_json, grpc_error, ok, status_value,
};
use crate::grpc_proto::{GeneralBlacklistListRequest, IpListRequest};

#[allow(dead_code)]
impl GoBackendClient {
    pub async fn list_general_blacklist(
        &self,
        page: i32,
        limit: i32,
        search: String,
    ) -> anyhow::Result<Value> {
        let mut client = self.security.clone();
        let result = match client
            .list_general_blacklist(self.request(GeneralBlacklistListRequest {
                page,
                limit,
                search,
            }))
            .await
        {
            Ok(response) => ok(general_blacklist_list_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("list_general_blacklist", result)
    }

    pub async fn check_general_blacklist(&self, ips: Vec<String>) -> anyhow::Result<Value> {
        let mut client = self.security.clone();
        let result = match client
            .check_general_blacklist(self.request(IpListRequest {
                ips,
                source: String::new(),
                comment: String::new(),
            }))
            .await
        {
            Ok(response) => ok(general_blacklist_status_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("check_general_blacklist", result)
    }

    pub async fn add_general_blacklist(
        &self,
        ips: Vec<String>,
        source: String,
        comment: String,
    ) -> anyhow::Result<Value> {
        let mut client = self.security.clone();
        let result = match client
            .add_general_blacklist(self.request(IpListRequest {
                ips,
                source,
                comment,
            }))
            .await
        {
            Ok(response) => ok(general_blacklist_mutation_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("add_general_blacklist", result)
    }

    pub async fn remove_general_blacklist(&self, ips: Vec<String>) -> anyhow::Result<Value> {
        let mut client = self.security.clone();
        let result = match client
            .remove_general_blacklist(self.request(IpListRequest {
                ips,
                source: String::new(),
                comment: String::new(),
            }))
            .await
        {
            Ok(response) => ok(general_blacklist_mutation_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("remove_general_blacklist", result)
    }
}
