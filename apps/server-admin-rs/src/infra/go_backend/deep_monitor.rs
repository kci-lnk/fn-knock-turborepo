use serde_json::{Value, json};
use tonic::Streaming;

use super::{GoBackendClient, grpc_error, ok, status_value};
use crate::grpc_proto::{
    DeepMonitorEvent, DeepMonitorEventRequest, DeepMonitorEventSummary, DeepMonitorExtendRequest,
    DeepMonitorListRequest, DeepMonitorPayloadChunk, DeepMonitorPayloadRequest, DeepMonitorQuery,
    DeepMonitorSession, DeepMonitorSessionRequest, DeepMonitorStartRequest,
    DeepMonitorWatchRequest, HeaderList,
};

#[allow(dead_code)]
impl GoBackendClient {
    pub async fn start_deep_monitor(
        &self,
        host: String,
        duration_seconds: i32,
    ) -> anyhow::Result<Value> {
        let mut client = self.deep_monitor.clone();
        let result = match client
            .start_session(self.request(DeepMonitorStartRequest {
                host,
                duration_seconds,
            }))
            .await
        {
            Ok(response) => ok(session_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("start_deep_monitor", result)
    }

    pub async fn extend_deep_monitor(
        &self,
        session_id: String,
        duration_seconds: i32,
    ) -> anyhow::Result<Value> {
        let mut client = self.deep_monitor.clone();
        let result = match client
            .extend_session(self.request(DeepMonitorExtendRequest {
                session_id,
                duration_seconds,
            }))
            .await
        {
            Ok(response) => ok(session_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("extend_deep_monitor", result)
    }

    pub async fn stop_deep_monitor(&self, session_id: String) -> anyhow::Result<Value> {
        let mut client = self.deep_monitor.clone();
        let result = match client
            .stop_session(self.request(DeepMonitorSessionRequest { session_id }))
            .await
        {
            Ok(response) => ok(session_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("stop_deep_monitor", result)
    }

    pub async fn list_deep_monitors(&self, include_expired: bool) -> anyhow::Result<Value> {
        let mut client = self.deep_monitor.clone();
        let result = match client
            .list_sessions(self.request(DeepMonitorListRequest { include_expired }))
            .await
        {
            Ok(response) => ok(json!({
                "items": response.into_inner().items.into_iter().map(session_json).collect::<Vec<_>>()
            })),
            Err(error) => grpc_error(error),
        };
        status_value("list_deep_monitors", result)
    }

    pub async fn query_deep_monitor_events(
        &self,
        query: DeepMonitorQuery,
    ) -> anyhow::Result<Value> {
        let mut client = self.deep_monitor.clone();
        let result = match client.query_events(self.request(query)).await {
            Ok(response) => {
                let response = response.into_inner();
                ok(json!({
                    "items": response.items.into_iter().map(summary_json).collect::<Vec<_>>(),
                    "next_cursor": response.next_cursor,
                    "has_more": response.has_more,
                }))
            }
            Err(error) => grpc_error(error),
        };
        status_value("query_deep_monitor_events", result)
    }

    pub async fn get_deep_monitor_event(
        &self,
        session_id: String,
        event_id: String,
    ) -> anyhow::Result<Value> {
        let mut client = self.deep_monitor.clone();
        let result = match client
            .get_event(self.request(DeepMonitorEventRequest {
                session_id,
                event_id,
            }))
            .await
        {
            Ok(response) => ok(event_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("get_deep_monitor_event", result)
    }

    pub async fn watch_deep_monitor_events(
        &self,
        session_id: String,
        after_sequence: u64,
    ) -> anyhow::Result<Streaming<DeepMonitorEventSummary>> {
        let mut client = self.deep_monitor.clone();
        Ok(client
            .watch_events(self.request(DeepMonitorWatchRequest {
                session_id,
                after_sequence,
            }))
            .await?
            .into_inner())
    }

    pub async fn stream_deep_monitor_payload(
        &self,
        session_id: String,
        event_id: String,
        part: String,
        offset: u64,
    ) -> anyhow::Result<Streaming<DeepMonitorPayloadChunk>> {
        let mut client = self.deep_monitor.clone();
        Ok(client
            .stream_payload(self.request(DeepMonitorPayloadRequest {
                session_id,
                event_id,
                part,
                offset,
            }))
            .await?
            .into_inner())
    }

    pub async fn stream_deep_monitor_archive(
        &self,
        session_id: String,
    ) -> anyhow::Result<Streaming<DeepMonitorPayloadChunk>> {
        let mut client = self.deep_monitor.clone();
        Ok(client
            .stream_session_archive(self.request(DeepMonitorSessionRequest { session_id }))
            .await?
            .into_inner())
    }

    pub async fn delete_deep_monitor(&self, session_id: String) -> anyhow::Result<Value> {
        let mut client = self.deep_monitor.clone();
        let result = match client
            .delete_session(self.request(DeepMonitorSessionRequest { session_id }))
            .await
        {
            Ok(_) => ok(Value::Null),
            Err(error) => grpc_error(error),
        };
        status_value("delete_deep_monitor", result)
    }
}

pub(crate) fn session_json(value: DeepMonitorSession) -> Value {
    json!({
        "id": value.id, "host": value.host, "state": value.state,
        "started_at": value.started_at, "deadline_at": value.deadline_at,
        "stopped_at": value.stopped_at, "stop_reason": value.stop_reason,
        "bytes_stored": value.bytes_stored, "event_count": value.event_count,
        "dropped_events": value.dropped_events, "quota_bytes": value.quota_bytes,
        "payload_limit_bytes": value.payload_limit_bytes,
    })
}

pub(crate) fn summary_json(value: DeepMonitorEventSummary) -> Value {
    json!({
        "id": value.id, "session_id": value.session_id, "sequence": value.sequence,
        "type": value.r#type, "time": value.time, "exchange_id": value.exchange_id,
        "connection_id": value.connection_id, "host": value.host, "method": value.method,
        "path": value.path, "status": value.status, "client_ip": value.client_ip,
        "identity": value.identity, "direction": value.direction,
        "payload_bytes": value.payload_bytes, "truncated": value.truncated,
        "notice": value.notice,
    })
}

fn headers_json(value: Option<HeaderList>) -> Value {
    Value::Array(
        value
            .unwrap_or_default()
            .headers
            .into_iter()
            .map(|header| {
                json!({
                    "name": header.name, "values": header.values
                })
            })
            .collect(),
    )
}

fn event_json(value: DeepMonitorEvent) -> Value {
    let timing = value.timing.map(|timing| {
        json!({
            "total_ms": timing.total_ms, "dns_ms": timing.dns_ms,
            "connect_ms": timing.connect_ms, "tls_ms": timing.tls_ms,
            "request_write_ms": timing.request_write_ms, "ttfb_ms": timing.ttfb_ms,
            "upstream_read_ms": timing.upstream_read_ms,
            "auth_ms": timing.auth_ms, "waf_ms": timing.waf_ms,
            "route_ms": timing.route_ms,
        })
    });
    let websocket_frame = value.websocket_frame.map(|frame| {
        json!({
            "direction": frame.direction, "fin": frame.fin, "rsv1": frame.rsv1,
            "rsv2": frame.rsv2, "rsv3": frame.rsv3, "opcode": frame.opcode,
            "masked": frame.masked, "mask_key": hex::encode(frame.mask_key),
            "payload_length": frame.payload_length, "close_code": frame.close_code,
            "close_reason": frame.close_reason, "compressed": frame.compressed,
        })
    });
    json!({
        "summary": value.summary.map(summary_json), "scheme": value.scheme,
        "protocol": value.protocol, "request_uri": value.request_uri,
        "upstream": value.upstream, "user_agent": value.user_agent,
        "referer": value.referer, "remote_addr": value.remote_addr,
        "auth_credential_id": value.auth_credential_id,
        "auth_credential_name": value.auth_credential_name,
        "auth_credential_method": value.auth_credential_method,
        "auth_linked_totp_id": value.auth_linked_totp_id,
        "auth_linked_totp_name": value.auth_linked_totp_name,
        "auth_decision": value.auth_decision, "route_type": value.route_type,
        "auth_rule_group_id": value.auth_rule_group_id,
        "auth_grant_state": value.auth_grant_state,
        "route_key": value.route_key, "tls_version": value.tls_version,
        "tls_cipher": value.tls_cipher, "tls_server_name": value.tls_server_name,
        "tls_alpn": value.tls_alpn,
        "client_request_headers": headers_json(value.client_request_headers),
        "upstream_request_headers": headers_json(value.upstream_request_headers),
        "upstream_response_headers": headers_json(value.upstream_response_headers),
        "client_response_headers": headers_json(value.client_response_headers),
        "payloads": value.payloads.into_iter().map(|payload| json!({
            "part": payload.part, "observed_bytes": payload.observed_bytes,
            "captured_bytes": payload.captured_bytes, "truncated": payload.truncated,
            "sha256": payload.sha256, "content_type": payload.content_type,
        })).collect::<Vec<_>>(),
        "timing": timing, "websocket_frame": websocket_frame,
        "websocket_subprotocol": value.websocket_subprotocol,
        "websocket_extensions": value.websocket_extensions, "error": value.error,
        "waf_trace_id": value.waf_trace_id, "waf_mode": value.waf_mode,
        "waf_rule_ids": value.waf_rule_ids, "waf_action": value.waf_action,
        "waf_bundle": value.waf_bundle, "waf_blocked": value.waf_blocked,
        "general_blacklist_blocked": value.general_blacklist_blocked,
        "client_ip_source": value.client_ip_source,
    })
}
