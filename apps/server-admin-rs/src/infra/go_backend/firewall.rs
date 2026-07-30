use reqwest::StatusCode;
use serde_json::Value;

use super::{
    GoBackendClient, bool_field, grpc_error, int_vec_any_field, parent_chains_from_body,
    parse_optional_compiled_ip_set, rpc_status_response, status_value, string_field,
    string_vec_any_field, string_vec_field,
};
use crate::grpc_proto::{
    IpRequest, IptablesInitRequest, SshFirewallClearRequest, SshFirewallSyncRequest,
    TcpRedirectRequest, WhitelistFirewallSyncRequest,
};

#[allow(dead_code)]
impl GoBackendClient {
    pub async fn allow_ip(&self, ip: &str) -> anyhow::Result<Value> {
        let mut client = self.firewall.clone();
        let result = match client
            .allow_ip(self.request(IpRequest { ip: ip.to_string() }))
            .await
        {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("allow_ip", result)
    }

    pub async fn remove_ip(&self, ip: &str) -> anyhow::Result<Value> {
        let mut client = self.firewall.clone();
        let result = match client
            .remove_ip(self.request(IpRequest { ip: ip.to_string() }))
            .await
        {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("remove_ip", result)
    }

    pub async fn sync_whitelist_firewall(&self, payload: &Value) -> anyhow::Result<Value> {
        let mut client = self.firewall.clone();
        let policy = parse_optional_compiled_ip_set(payload.get("policy"))?;
        let result = match client
            .sync_whitelist_firewall(self.request(WhitelistFirewallSyncRequest {
                policy_id: string_field(payload, "policy_id"),
                policy,
            }))
            .await
        {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("sync_whitelist_firewall", result)
    }

    pub async fn init_iptables(&self, payload: &Value) -> anyhow::Result<Value> {
        let mut client = self.firewall.clone();
        let result = match client
            .init_iptables(self.request(IptablesInitRequest {
                chain_name: string_field(payload, "chain_name"),
                parent_chains: parent_chains_from_body(payload),
                exempt_ports: string_vec_any_field(payload, "exempt_ports"),
            }))
            .await
        {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("init_iptables", result)
    }

    pub async fn clean_iptables(&self) -> anyhow::Result<Value> {
        let mut client = self.firewall.clone();
        let result = match client.clean_iptables(self.request(())).await {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("clean_iptables", result)
    }

    pub async fn sync_ssh_firewall(&self, payload: &Value) -> anyhow::Result<Value> {
        let mut client = self.firewall.clone();
        let policy = parse_optional_compiled_ip_set(payload.get("policy"))?;
        let result = match client
            .sync_ssh_firewall(self.request(SshFirewallSyncRequest {
                chain_name: string_field(payload, "chain_name"),
                parent_chains: parent_chains_from_body(payload),
                ports: int_vec_any_field(payload, "ports"),
                allowed_cidrs: string_vec_field(payload, "allowed_cidrs"),
                blocked_ips: string_vec_field(payload, "blocked_ips"),
                include_local_cidrs: bool_field(payload, "include_local_cidrs", false),
                policy_id: string_field(payload, "policy_id"),
                policy,
            }))
            .await
        {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("sync_ssh_firewall", result)
    }

    pub async fn clear_ssh_firewall(&self, payload: &Value) -> anyhow::Result<Value> {
        let mut client = self.firewall.clone();
        let result = match client
            .clear_ssh_firewall(self.request(SshFirewallClearRequest {
                chain_name: string_field(payload, "chain_name"),
                parent_chains: parent_chains_from_body(payload),
            }))
            .await
        {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("clear_ssh_firewall", result)
    }

    pub async fn clear_tcp_redirect(
        &self,
        listen_port: i64,
        target_port: i64,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.firewall.clone();
        match client
            .clear_tcp_redirect(self.request(TcpRedirectRequest {
                listen_port: i32::try_from(listen_port).unwrap_or(0),
                target_port: i32::try_from(target_port).unwrap_or(0),
            }))
            .await
        {
            Ok(response) => Ok(rpc_status_response(response.into_inner())),
            Err(error) => Ok(grpc_error(error)),
        }
    }
}
