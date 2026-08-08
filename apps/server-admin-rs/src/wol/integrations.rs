use rand::random_range;
use rumqttc::{
    AsyncClient, ConnectionError, Event, Incoming, MqttOptions, QoS, SubscribeFilter, Transport,
    mqttbytes::v4::ConnectReturnCode,
};
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::{task::JoinSet, time};
use tokio_util::sync::CancellationToken;

use crate::{state::AppState, time_utils};

use super::{
    secrets::{IntegrationCredentialKind, secret_store},
    service::{WakeSource, wake_target},
    store::{TargetRecord, list_targets},
};

const BEMFA_HOST: &str = "bemfa.com";
const BEMFA_TLS_PORT: u16 = 9503;
const BLINKER_AUTH_URL: &str = "https://iot.diandeng.tech/api/v1/user/device/diy/auth";
const BLINKER_HEARTBEAT_URL: &str = "https://iot.diandeng.tech/api/v1/user/device/heartbeat";
const BLINKER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(599);
const BLINKER_AUTH_REFRESH_MIN_INTERVAL: Duration = Duration::from_secs(60);
const MAX_BLINKER_REPLY_BYTES: usize = 1024;
const BLINKER_REPLY_INTERVAL: Duration = Duration::from_secs(1);
const INTEGRATION_CONFIG_RETRY_INTERVAL: Duration = Duration::from_secs(5);
static VERIFIED_MQTT_TLS_CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IntegrationRuntimeView {
    pub state: String,
    pub last_connected_at: Option<String>,
    pub last_message_at: Option<String>,
    pub last_error: Option<String>,
}

impl IntegrationRuntimeView {
    pub(super) fn disabled() -> Self {
        Self {
            state: "disabled".to_string(),
            last_connected_at: None,
            last_message_at: None,
            last_error: None,
        }
    }

    pub(super) fn credential_missing() -> Self {
        Self {
            state: "credential_missing".to_string(),
            ..Self::disabled()
        }
    }
}

fn runtime_key(target_id: &str, provider: &str) -> String {
    format!("{target_id}:{provider}")
}

pub(super) async fn runtime_view(
    state: &AppState,
    target_id: &str,
    provider: &str,
    enabled: bool,
    credential_configured: bool,
) -> IntegrationRuntimeView {
    if !enabled {
        return IntegrationRuntimeView::disabled();
    }
    if !credential_configured {
        return IntegrationRuntimeView::credential_missing();
    }
    state
        .wol_integration_status
        .read()
        .await
        .get(&runtime_key(target_id, provider))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or(IntegrationRuntimeView {
            state: "connecting".to_string(),
            last_connected_at: None,
            last_message_at: None,
            last_error: None,
        })
}

pub(super) async fn set_runtime(
    state: &AppState,
    target_id: &str,
    provider: &str,
    runtime_state: &str,
    connected: bool,
    message_received: bool,
    error: Option<&str>,
) {
    let key = runtime_key(target_id, provider);
    let mut statuses = state.wol_integration_status.write().await;
    let previous = statuses.get(&key);
    let now = time_utils::now_iso();
    let last_connected_at = if connected {
        Some(now.clone())
    } else {
        previous
            .and_then(|value| value.get("lastConnectedAt"))
            .cloned()
            .unwrap_or(Value::Null)
            .as_str()
            .map(ToOwned::to_owned)
    };
    let last_message_at = if message_received {
        Some(now)
    } else {
        previous
            .and_then(|value| value.get("lastMessageAt"))
            .cloned()
            .unwrap_or(Value::Null)
            .as_str()
            .map(ToOwned::to_owned)
    };
    let previous_error = previous
        .and_then(|value| value.get("lastError"))
        .and_then(Value::as_str);
    let last_error = next_runtime_error(previous_error, runtime_state, error);
    statuses.insert(
        key,
        json!({
            "state": runtime_state,
            "lastConnectedAt": last_connected_at,
            "lastMessageAt": last_message_at,
            "lastError": last_error,
        }),
    );
}

fn next_runtime_error(
    previous: Option<&str>,
    runtime_state: &str,
    error: Option<&str>,
) -> Option<String> {
    match error {
        Some(error) => Some(sanitize_runtime_error(error)),
        None if runtime_state == "connected" => None,
        None => previous.map(ToOwned::to_owned),
    }
}

pub(super) async fn remove_runtime(state: &AppState, target_id: &str) {
    let mut statuses = state.wol_integration_status.write().await;
    statuses.remove(&runtime_key(target_id, "blinker"));
    statuses.remove(&runtime_key(target_id, "bemfa"));
}

fn sanitize_runtime_error(error: &str) -> String {
    let mut result = error
        .chars()
        .filter(|ch| !ch.is_control())
        .take(240)
        .collect::<String>();
    let lowercase = result.to_ascii_lowercase();
    let first_secret = [
        "password=",
        "iottoken=",
        "iot_token=",
        "privatekey=",
        "devicekey=",
        "authkey=",
        "client_id=",
        "clientid=",
        "token=",
        "key=",
        "authkey%3d",
        "token%3d",
        "key%3d",
    ]
    .into_iter()
    .filter_map(|marker| lowercase.find(marker).map(|index| (index, marker.len())))
    .min_by_key(|(index, _)| *index);
    if let Some((index, marker_length)) = first_secret {
        result.truncate(index + marker_length);
        result.push_str("[redacted]");
    }
    result
}

pub(super) fn start_integration_tasks(state: AppState) {
    tokio::spawn(async move { integration_supervisor(state).await });
}

async fn integration_supervisor(state: AppState) {
    let mut reload = state.wol_runtime_reload.subscribe();
    loop {
        if state.shutdown.is_cancelled() {
            return;
        }
        state.wol_integration_status.write().await.clear();
        let workers_cancel = state.shutdown.child_token();
        let mut workers = JoinSet::new();
        let retry_configuration = match super::feature_enabled_for_state(&state).await {
            Ok(true) => !start_configured_workers(&state, &workers_cancel, &mut workers).await,
            Ok(false) => false,
            Err(error) => {
                tracing::warn!(%error, "failed to load WoL integration configuration");
                true
            }
        };
        let worker_failed = tokio::select! {
            _ = state.shutdown.cancelled() => {
                workers_cancel.cancel();
                workers.abort_all();
                return;
            }
            _ = reload.changed() => {
                false
            }
            _ = time::sleep(INTEGRATION_CONFIG_RETRY_INTERVAL), if retry_configuration => {
                false
            }
            result = workers.join_next(), if !workers.is_empty() => {
                match result {
                    Some(Ok(())) => tracing::warn!("WoL integration worker stopped unexpectedly"),
                    Some(Err(error)) => {
                        tracing::warn!(
                            cancelled = error.is_cancelled(),
                            panicked = error.is_panic(),
                            "WoL integration worker failed"
                        )
                    }
                    None => {}
                }
                true
            }
        };
        workers_cancel.cancel();
        workers.abort_all();
        while workers.join_next().await.is_some() {}
        if worker_failed {
            tokio::select! {
                _ = state.shutdown.cancelled() => return,
                _ = reload.changed() => {}
                _ = time::sleep(INTEGRATION_CONFIG_RETRY_INTERVAL) => {}
            }
        }
    }
}

#[derive(Clone)]
struct BemfaBinding {
    target_id: String,
    topic: String,
}

struct BemfaGroup {
    private_key: String,
    skip_tls_verify: bool,
    bindings: Vec<BemfaBinding>,
}

async fn start_configured_workers(
    state: &AppState,
    cancel: &CancellationToken,
    workers: &mut JoinSet<()>,
) -> bool {
    let targets = match list_targets(state).await {
        Ok(targets) => targets,
        Err(error) => {
            tracing::warn!(%error, "failed to load WoL integration Targets");
            return false;
        }
    };
    let secrets = secret_store(state);
    let mut bemfa_groups = HashMap::<(String, bool), BemfaGroup>::new();
    for target in targets.into_iter().filter(|target| target.enabled) {
        if target.integrations.blinker.enabled && target.integrations.bemfa.enabled {
            let error = "Only one of Blinker or Bemfa can be enabled for a Target";
            set_runtime(
                state,
                &target.id,
                "blinker",
                "error",
                false,
                false,
                Some(error),
            )
            .await;
            set_runtime(
                state,
                &target.id,
                "bemfa",
                "error",
                false,
                false,
                Some(error),
            )
            .await;
            continue;
        }
        if target.integrations.blinker.enabled {
            match secrets.read_integration(&target.id, IntegrationCredentialKind::Blinker) {
                Ok(Some(key)) => match String::from_utf8(key) {
                    Ok(key) => {
                        let state = state.clone();
                        let cancel = cancel.child_token();
                        let blinker_target = target.clone();
                        workers.spawn(async move {
                            blinker_worker(state, blinker_target, key, cancel).await
                        });
                    }
                    Err(_) => {
                        set_runtime(
                            state,
                            &target.id,
                            "blinker",
                            "error",
                            false,
                            false,
                            Some("Stored Blinker credential is invalid"),
                        )
                        .await;
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    set_runtime(
                        state,
                        &target.id,
                        "blinker",
                        "error",
                        false,
                        false,
                        Some(&error),
                    )
                    .await;
                }
            }
        }
        if target.integrations.bemfa.enabled && !target.integrations.bemfa.topic.is_empty() {
            match secrets.read_integration(&target.id, IntegrationCredentialKind::Bemfa) {
                Ok(Some(key)) => match String::from_utf8(key) {
                    Ok(private_key) => {
                        let fingerprint = hex::encode(Sha256::digest(private_key.as_bytes()));
                        bemfa_groups
                            .entry((fingerprint, target.integrations.bemfa.skip_tls_verify))
                            .or_insert_with(|| BemfaGroup {
                                private_key,
                                skip_tls_verify: target.integrations.bemfa.skip_tls_verify,
                                bindings: Vec::new(),
                            })
                            .bindings
                            .push(BemfaBinding {
                                target_id: target.id,
                                topic: target.integrations.bemfa.topic,
                            });
                    }
                    Err(_) => {
                        set_runtime(
                            state,
                            &target.id,
                            "bemfa",
                            "error",
                            false,
                            false,
                            Some("Stored Bemfa credential is invalid"),
                        )
                        .await;
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    set_runtime(
                        state,
                        &target.id,
                        "bemfa",
                        "error",
                        false,
                        false,
                        Some(&error),
                    )
                    .await;
                }
            }
        }
    }
    for group in bemfa_groups.into_values() {
        let state = state.clone();
        let cancel = cancel.child_token();
        workers.spawn(async move { bemfa_worker(state, group, cancel).await });
    }
    true
}

async fn bemfa_worker(state: AppState, group: BemfaGroup, cancel: CancellationToken) {
    let mut attempt = 0_u32;
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let runtime_state = if attempt == 0 {
            "connecting"
        } else {
            "reconnecting"
        };
        for binding in &group.bindings {
            set_runtime(
                &state,
                &binding.target_id,
                "bemfa",
                runtime_state,
                false,
                false,
                None,
            )
            .await;
        }
        let result = bemfa_session(&state, &group, &cancel).await;
        if cancel.is_cancelled() {
            return;
        }
        let failure = result.err().unwrap_or_else(|| SessionFailure {
            message: "Bemfa connection ended".to_string(),
            connected: false,
            auth_failed: false,
        });
        for binding in &group.bindings {
            set_runtime(
                &state,
                &binding.target_id,
                "bemfa",
                "error",
                false,
                false,
                Some(&failure.message),
            )
            .await;
        }
        attempt = next_retry_attempt(attempt, failure.connected);
        if wait_for_retry(&cancel, attempt).await {
            return;
        }
    }
}

async fn bemfa_session(
    state: &AppState,
    group: &BemfaGroup,
    cancel: &CancellationToken,
) -> Result<(), SessionFailure> {
    let mut options = MqttOptions::new(&group.private_key, BEMFA_HOST, BEMFA_TLS_PORT);
    options
        .set_keep_alive(Duration::from_secs(60))
        .set_clean_session(true)
        .set_max_packet_size(4096, 4096)
        .set_transport(mqtt_tls_transport(group.skip_tls_verify));
    let (client, mut eventloop) = AsyncClient::new(options, 32);
    let topics = group
        .bindings
        .iter()
        .map(|binding| SubscribeFilter::new(binding.topic.clone(), QoS::AtLeastOnce))
        .collect::<Vec<_>>();
    client
        .subscribe_many(topics)
        .await
        .map_err(|_| SessionFailure::new("Failed to subscribe to Bemfa topics"))?;
    let topic_targets = group
        .bindings
        .iter()
        .map(|binding| (binding.topic.as_str(), binding.target_id.as_str()))
        .collect::<HashMap<_, _>>();
    let mut status_updates = state.wol_status_updates.subscribe();
    let mut connected = false;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = client.disconnect().await;
                return Ok(());
            }
            update = status_updates.recv() => {
                if let Ok(update) = update {
                    publish_bemfa_status(&client, group, &update)
                        .await
                        .map_err(|message| SessionFailure::after_connection(message, connected))?;
                }
            }
            event = eventloop.poll() => match event {
                Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                    connected = true;
                    for binding in &group.bindings {
                        set_runtime(state, &binding.target_id, "bemfa", "connected", true, false, None).await;
                    }
                }
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    let Some(target_id) = topic_targets.get(publish.topic.as_str()).copied() else { continue; };
                    set_runtime(state, target_id, "bemfa", "connected", false, true, None).await;
                    if bemfa_payload_is_on(&publish.payload) {
                        if let Err(error) = wake_target(state, target_id, WakeSource::Bemfa).await
                            && error.status != axum::http::StatusCode::TOO_MANY_REQUESTS
                        {
                            set_runtime(state, target_id, "bemfa", "error", false, false, Some(&error.message)).await;
                        }
                    } else if bemfa_payload_is_off(&publish.payload) {
                        publish_current_bemfa_status(state, &client, target_id, publish.topic.as_str())
                            .await
                            .map_err(|message| SessionFailure::after_connection(message, connected))?;
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    return Err(SessionFailure::after_connection(
                        format!("Bemfa MQTT connection failed: {}", connection_error_label(&error)),
                        connected,
                    ));
                }
            }
        }
    }
}

async fn publish_bemfa_status(
    client: &AsyncClient,
    group: &BemfaGroup,
    update: &Value,
) -> Result<(), String> {
    let Some(target_id) = update.get("targetId").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(payload) = known_status_payload(update.get("state").and_then(Value::as_str)) else {
        return Ok(());
    };
    let Some(binding) = group
        .bindings
        .iter()
        .find(|binding| binding.target_id == target_id)
    else {
        return Ok(());
    };
    client
        .publish(
            format!("{}/up", binding.topic),
            QoS::AtLeastOnce,
            false,
            payload,
        )
        .await
        .map_err(|_| "Failed to publish Bemfa state".to_string())
}

async fn publish_current_bemfa_status(
    state: &AppState,
    client: &AsyncClient,
    target_id: &str,
    topic: &str,
) -> Result<(), String> {
    let status = super::status::status_view(state, target_id)
        .await
        .map_err(|_| "Failed to load Target state".to_string())?;
    let Some(payload) = known_status_payload(Some(status.state.as_str())) else {
        return Ok(());
    };
    client
        .publish(format!("{topic}/up"), QoS::AtLeastOnce, false, payload)
        .await
        .map_err(|_| "Failed to publish Bemfa state".to_string())
}

fn known_status_payload(state: Option<&str>) -> Option<&'static str> {
    match state {
        Some("online") => Some("on"),
        Some("offline") => Some("off"),
        _ => None,
    }
}

fn bemfa_payload_command(payload: &[u8]) -> Option<&'static str> {
    let payload = std::str::from_utf8(payload).ok()?.trim();
    let command = payload.split('#').next()?.trim();
    if command.eq_ignore_ascii_case("on") {
        Some("on")
    } else if command.eq_ignore_ascii_case("off") {
        Some("off")
    } else {
        None
    }
}

fn bemfa_payload_is_on(payload: &[u8]) -> bool {
    bemfa_payload_command(payload) == Some("on")
}

fn bemfa_payload_is_off(payload: &[u8]) -> bool {
    bemfa_payload_command(payload) == Some("off")
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlinkerAuth {
    broker: String,
    device_name: String,
    host: String,
    #[serde(deserialize_with = "deserialize_blinker_port")]
    port: u16,
    iot_id: String,
    iot_token: String,
    uuid: String,
}

#[derive(Debug)]
struct SessionFailure {
    message: String,
    connected: bool,
    auth_failed: bool,
}

impl SessionFailure {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            connected: false,
            auth_failed: false,
        }
    }

    fn after_connection(message: impl Into<String>, connected: bool) -> Self {
        Self {
            message: message.into(),
            connected,
            auth_failed: false,
        }
    }
}

fn deserialize_blinker_port<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PortValue {
        Number(u16),
        String(String),
    }

    match PortValue::deserialize(deserializer)? {
        PortValue::Number(port) => Ok(port),
        PortValue::String(port) => port
            .parse::<u16>()
            .map_err(|_| de::Error::custom("Blinker MQTT port is invalid")),
    }
}

#[derive(Deserialize)]
struct BlinkerEnvelope {
    #[serde(rename = "fromDevice")]
    from_device: String,
    data: Value,
}

async fn blinker_worker(
    state: AppState,
    target: TargetRecord,
    device_key: String,
    cancel: CancellationToken,
) {
    let mut auth = None;
    let mut auth_fetched_at = None;
    let mut attempt = 0_u32;
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let runtime_state = if attempt == 0 {
            "connecting"
        } else {
            "reconnecting"
        };
        set_runtime(
            &state,
            &target.id,
            "blinker",
            runtime_state,
            false,
            false,
            None,
        )
        .await;
        if auth.is_none() {
            match fetch_blinker_auth(&device_key, target.integrations.blinker.skip_tls_verify).await
            {
                Ok(value) => {
                    auth = Some(value);
                    auth_fetched_at = Some(Instant::now());
                }
                Err(error) => {
                    set_runtime(
                        &state,
                        &target.id,
                        "blinker",
                        "error",
                        false,
                        false,
                        Some(&error),
                    )
                    .await;
                    attempt = attempt.saturating_add(1);
                    if wait_for_retry(&cancel, attempt).await {
                        return;
                    }
                    continue;
                }
            }
        }
        let Some(current_auth) = auth.clone() else {
            continue;
        };
        let result = blinker_session(&state, &target, &device_key, &current_auth, &cancel).await;
        if cancel.is_cancelled() {
            return;
        }
        let failure = result.err().unwrap_or_else(|| SessionFailure {
            message: "Blinker connection ended".to_string(),
            connected: false,
            auth_failed: false,
        });
        set_runtime(
            &state,
            &target.id,
            "blinker",
            "error",
            false,
            false,
            Some(&failure.message),
        )
        .await;
        if should_refresh_auth(auth_fetched_at, Instant::now(), failure.auth_failed) {
            auth = None;
        }
        attempt = next_retry_attempt(attempt, failure.connected);
        if wait_for_retry(&cancel, attempt).await {
            return;
        }
    }
}

async fn fetch_blinker_auth(
    device_key: &str,
    skip_tls_verify: bool,
) -> Result<BlinkerAuth, String> {
    let client = integration_http_client(skip_tls_verify)?;
    let response = client
        .get(BLINKER_AUTH_URL)
        .query(&[("authKey", device_key), ("protocol", "mqtts")])
        .send()
        .await
        .map_err(|error| request_error("Blinker auth request", error))?;
    if !response.status().is_success() {
        return Err(format!("Blinker auth returned HTTP {}", response.status()));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|_| "Blinker auth response is invalid".to_string())?;
    parse_blinker_auth_response(value)
}

fn parse_blinker_auth_response(value: Value) -> Result<BlinkerAuth, String> {
    if !blinker_response_succeeded(&value) {
        return Err("Blinker rejected the device key".to_string());
    }
    let mut auth = serde_json::from_value::<BlinkerAuth>(
        value
            .get("detail")
            .cloned()
            .ok_or_else(|| "Blinker auth response is missing connection details".to_string())?,
    )
    .map_err(|_| "Blinker auth connection details are invalid".to_string())?;
    if auth.broker != "blinker" {
        return Err("Blinker auth returned an unsupported MQTT broker".to_string());
    }
    if !is_safe_blinker_identifier(&auth.device_name)
        || auth.port == 0
        || !is_safe_blinker_value(&auth.iot_id)
        || !is_safe_blinker_value(&auth.iot_token)
        || !is_safe_blinker_value(&auth.uuid)
    {
        return Err("Blinker auth connection details are incomplete".to_string());
    }
    auth.host = normalize_blinker_mqtt_host(&auth.host, auth.port)?;
    Ok(auth)
}

fn is_safe_blinker_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_safe_blinker_value(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn normalize_blinker_mqtt_host(value: &str, port: u16) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 512
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err("Blinker auth MQTT host is invalid".to_string());
    }
    if !value.contains("://") {
        if value.chars().any(|ch| matches!(ch, '/' | '?' | '#' | '@')) {
            return Err("Blinker auth MQTT host is invalid".to_string());
        }
        return Ok(value.to_string());
    }
    let url =
        reqwest::Url::parse(value).map_err(|_| "Blinker auth MQTT host is invalid".to_string())?;
    if url.scheme() != "mqtts"
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port().is_some_and(|url_port| url_port != port)
    {
        return Err("Blinker auth did not return a valid MQTT TLS endpoint".to_string());
    }
    url.host_str()
        .filter(|host| !host.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Blinker auth MQTT host is invalid".to_string())
}

async fn blinker_session(
    state: &AppState,
    target: &TargetRecord,
    device_key: &str,
    auth: &BlinkerAuth,
    cancel: &CancellationToken,
) -> Result<(), SessionFailure> {
    let mut options = MqttOptions::new(&auth.device_name, &auth.host, auth.port);
    options
        .set_credentials(&auth.iot_id, &auth.iot_token)
        .set_keep_alive(Duration::from_secs(60))
        .set_clean_session(true)
        .set_max_packet_size(4096, 4096)
        .set_transport(mqtt_tls_transport(
            target.integrations.blinker.skip_tls_verify,
        ));
    let (client, mut eventloop) = AsyncClient::new(options, 32);
    let receive_topic = format!("/device/{}/r", auth.device_name);
    let send_topic = format!("/device/{}/s", auth.device_name);
    client
        .subscribe(receive_topic.clone(), QoS::AtMostOnce)
        .await
        .map_err(|_| SessionFailure::new("Failed to subscribe to Blinker device topic"))?;
    let mut heartbeat = time::interval(BLINKER_HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let mut heartbeat_started = false;
    let mut last_reply = None;
    let mut connected = false;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = client.disconnect().await;
                return Ok(());
            }
            _ = heartbeat.tick(), if heartbeat_started => {
                match send_blinker_heartbeat(device_key, auth, target.integrations.blinker.skip_tls_verify).await {
                    Ok(()) => set_runtime(state, &target.id, "blinker", "connected", false, false, None).await,
                    Err(error) => set_runtime(state, &target.id, "blinker", "connected", false, false, Some(&error)).await,
                }
            }
            event = eventloop.poll() => match event {
                Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                    connected = true;
                    set_runtime(state, &target.id, "blinker", "connected", true, false, None).await;
                    match send_blinker_heartbeat(device_key, auth, target.integrations.blinker.skip_tls_verify).await {
                        Ok(()) => set_runtime(state, &target.id, "blinker", "connected", false, false, None).await,
                        Err(error) => set_runtime(state, &target.id, "blinker", "connected", false, false, Some(&error)).await,
                    }
                    heartbeat.reset();
                    heartbeat_started = true;
                }
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    if publish.topic != receive_topic { continue; }
                    let Some(command) = parse_blinker_command(&publish.payload, &auth.uuid) else { continue; };
                    set_runtime(state, &target.id, "blinker", "connected", false, true, None).await;
                    let mut reply = None;
                    match command {
                        BlinkerCommand::SwitchOn if target.integrations.blinker.bind_component => {
                            if let Err(error) = wake_target(state, &target.id, WakeSource::Blinker).await
                                && error.status != axum::http::StatusCode::TOO_MANY_REQUESTS
                            {
                                set_runtime(state, &target.id, "blinker", "error", false, false, Some(&error.message)).await;
                            }
                            reply = Some(blinker_state_reply(state, target, auth).await);
                        }
                        BlinkerCommand::SwitchOff if target.integrations.blinker.bind_component => {
                            reply = Some(blinker_state_reply(state, target, auth).await);
                        }
                        BlinkerCommand::GetState => {
                            reply = Some(blinker_state_reply(state, target, auth).await);
                        }
                        _ => {}
                    }
                    if let Some(reply) = reply {
                        publish_blinker_reply(&client, &send_topic, reply, &mut last_reply, cancel).await?;
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    let auth_failed = is_mqtt_auth_failure(&error);
                    return Err(SessionFailure {
                        message: format!("Blinker MQTT connection failed: {}", connection_error_label(&error)),
                        connected,
                        auth_failed,
                    });
                }
            }
        }
    }
}

async fn send_blinker_heartbeat(
    device_key: &str,
    auth: &BlinkerAuth,
    skip_tls_verify: bool,
) -> Result<(), String> {
    let response = integration_http_client(skip_tls_verify)?
        .get(BLINKER_HEARTBEAT_URL)
        .query(&[
            ("deviceName", auth.device_name.as_str()),
            ("key", device_key),
            ("heartbeat", "600"),
        ])
        .send()
        .await
        .map_err(|error| request_error("Blinker heartbeat", error))?;
    if !response.status().is_success() {
        return Err(format!(
            "Blinker heartbeat returned HTTP {}",
            response.status()
        ));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|_| "Blinker heartbeat response is invalid".to_string())?;
    parse_blinker_heartbeat_response(&value)
}

fn blinker_response_succeeded(value: &Value) -> bool {
    value
        .get("message")
        .is_some_and(|message| message.as_u64() == Some(1000) || message.as_str() == Some("1000"))
}

fn parse_blinker_heartbeat_response(value: &Value) -> Result<(), String> {
    if blinker_response_succeeded(value) {
        Ok(())
    } else {
        Err("Blinker heartbeat was rejected".to_string())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BlinkerCommand {
    SwitchOn,
    SwitchOff,
    GetState,
}

fn parse_blinker_command(payload: &[u8], owner_uuid: &str) -> Option<BlinkerCommand> {
    if payload.len() > MAX_BLINKER_REPLY_BYTES {
        return None;
    }
    let envelope = serde_json::from_slice::<BlinkerEnvelope>(payload).ok()?;
    if envelope.from_device != owner_uuid {
        return None;
    }
    let data = match envelope.data {
        Value::String(value) => serde_json::from_str::<Value>(&value).ok()?,
        value => value,
    };
    let object = data.as_object()?;
    match object.get("switch").and_then(Value::as_str) {
        Some(value) if value.eq_ignore_ascii_case("on") => Some(BlinkerCommand::SwitchOn),
        Some(value) if value.eq_ignore_ascii_case("off") => Some(BlinkerCommand::SwitchOff),
        _ if object.get("get").and_then(Value::as_str) == Some("state") => {
            Some(BlinkerCommand::GetState)
        }
        _ => None,
    }
}

async fn blinker_state_reply(state: &AppState, target: &TargetRecord, auth: &BlinkerAuth) -> Value {
    let status = super::status::status_view(state, &target.id).await.ok();
    let component_state = target
        .integrations
        .blinker
        .bind_component
        .then(|| {
            status
                .as_ref()
                .and_then(|status| known_status_payload(Some(status.state.as_str())))
        })
        .flatten();
    build_blinker_state_reply(auth, component_state)
}

fn build_blinker_state_reply(auth: &BlinkerAuth, component_state: Option<&str>) -> Value {
    let mut data = Map::new();
    data.insert("state".to_string(), json!("online"));
    if let Some(component_state) = component_state {
        data.insert("switch".to_string(), json!({ "swi": component_state }));
    }
    json!({
        "deviceType": "OwnApp",
        "data": data,
        "fromDevice": auth.device_name,
        "toDevice": auth.uuid,
    })
}

async fn publish_blinker_reply(
    client: &AsyncClient,
    topic: &str,
    reply: Value,
    last_reply: &mut Option<Instant>,
    cancel: &CancellationToken,
) -> Result<(), SessionFailure> {
    let now = Instant::now();
    let delay = blinker_reply_delay(*last_reply, now);
    if !delay.is_zero() {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            _ = time::sleep(delay) => {}
        }
    }
    let payload = serde_json::to_vec(&reply)
        .map_err(|_| SessionFailure::after_connection("Failed to encode Blinker reply", true))?;
    if payload.len() > MAX_BLINKER_REPLY_BYTES {
        return Err(SessionFailure::after_connection(
            "Blinker reply exceeded 1024 bytes",
            true,
        ));
    }
    client
        .publish(topic, QoS::AtMostOnce, false, payload)
        .await
        .map_err(|_| SessionFailure::after_connection("Failed to publish Blinker reply", true))?;
    *last_reply = Some(Instant::now());
    Ok(())
}

fn blinker_reply_delay(last_reply: Option<Instant>, now: Instant) -> Duration {
    last_reply
        .map(|last| BLINKER_REPLY_INTERVAL.saturating_sub(now.saturating_duration_since(last)))
        .unwrap_or_default()
}

fn integration_http_client(skip_tls_verify: bool) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(skip_tls_verify)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Failed to initialize integration HTTP client".to_string())
}

fn request_error(action: &str, error: reqwest::Error) -> String {
    let reason = if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_body() || error.is_decode() {
        "response failed"
    } else {
        "request failed"
    };
    format!("{action} failed: {reason}")
}

fn should_refresh_auth(fetched_at: Option<Instant>, now: Instant, auth_failed: bool) -> bool {
    auth_failed
        && fetched_at.is_none_or(|fetched_at| {
            now.saturating_duration_since(fetched_at) >= BLINKER_AUTH_REFRESH_MIN_INTERVAL
        })
}

fn next_retry_attempt(attempt: u32, connected: bool) -> u32 {
    if connected {
        1
    } else {
        attempt.saturating_add(1)
    }
}

fn is_mqtt_auth_failure(error: &ConnectionError) -> bool {
    matches!(
        error,
        ConnectionError::ConnectionRefused(
            ConnectReturnCode::BadUserNamePassword | ConnectReturnCode::NotAuthorized
        )
    )
}

fn connection_error_label(error: &ConnectionError) -> &'static str {
    match error {
        ConnectionError::ConnectionRefused(_) => "broker refused the connection",
        ConnectionError::Tls(_) => "TLS verification or handshake failed",
        ConnectionError::Io(_) => "network I/O failed",
        ConnectionError::NetworkTimeout | ConnectionError::FlushTimeout => "network timed out",
        ConnectionError::MqttState(_) | ConnectionError::NotConnAck(_) => "MQTT protocol error",
        ConnectionError::RequestsDone => "MQTT client stopped",
    }
}

async fn wait_for_retry(cancel: &CancellationToken, attempt: u32) -> bool {
    let exponent = attempt.min(6);
    let base_ms = 1_000_u64.saturating_mul(1_u64 << exponent);
    let delay = Duration::from_millis(base_ms.min(60_000) + random_range(0..=750));
    tokio::select! {
        _ = cancel.cancelled() => true,
        _ = time::sleep(delay) => false,
    }
}

fn mqtt_tls_transport(skip_tls_verify: bool) -> Transport {
    ensure_rustls_crypto_provider();
    use rumqttc::tokio_rustls::rustls::{ClientConfig, RootCertStore};
    if !skip_tls_verify {
        // rumqttc's default configuration calls `expect` when loading native
        // roots. Some platforms can return usable certificates together with
        // non-fatal read errors, so build the store ourselves and retain every
        // valid certificate without letting a host certificate issue panic the
        // service. An empty store safely fails the later TLS handshake.
        let config = VERIFIED_MQTT_TLS_CONFIG
            .get_or_init(|| {
                let native_certs = rustls_native_certs::load_native_certs();
                let error_count = native_certs.errors.len();
                let mut roots = RootCertStore::empty();
                let (accepted, ignored) = roots.add_parsable_certificates(native_certs.certs);
                if error_count > 0 || ignored > 0 {
                    tracing::warn!(
                        error_count,
                        ignored,
                        accepted,
                        "some native certificates could not be loaded for WoL integrations"
                    );
                }
                Arc::new(
                    ClientConfig::builder()
                        .with_root_certificates(roots)
                        .with_no_client_auth(),
                )
            })
            .clone();
        return Transport::tls_with_config(rumqttc::TlsConfiguration::Rustls(config));
    }
    use rumqttc::tokio_rustls::rustls::client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    };
    use rumqttc::tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rumqttc::tokio_rustls::rustls::{DigitallySignedStruct, SignatureScheme};

    #[derive(Debug)]
    struct SkipServerVerification;

    impl ServerCertVerifier for SkipServerVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rumqttc::tokio_rustls::rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rumqttc::tokio_rustls::rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rumqttc::tokio_rustls::rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ED25519,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PSS_SHA384,
                SignatureScheme::RSA_PSS_SHA512,
                SignatureScheme::RSA_PKCS1_SHA256,
                SignatureScheme::RSA_PKCS1_SHA384,
                SignatureScheme::RSA_PKCS1_SHA512,
            ]
        }
    }

    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();
    Transport::tls_with_config(rumqttc::TlsConfiguration::Rustls(Arc::new(config)))
}

fn ensure_rustls_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_errors_are_bounded_and_redacted() {
        assert_eq!(
            sanitize_runtime_error("auth failed: token=secret-value"),
            "auth failed: token=[redacted]"
        );
        assert_eq!(
            sanitize_runtime_error("request URL?AuthKey=secret-value&protocol=mqtts"),
            "request URL?AuthKey=[redacted]"
        );
        assert_eq!(
            sanitize_runtime_error("request URL?authKey%3Dsecret-value"),
            "request URL?authKey%3D[redacted]"
        );
        assert!(sanitize_runtime_error(&"x".repeat(400)).len() <= 240);
    }

    #[test]
    fn connected_runtime_clears_stale_errors_but_reconnecting_preserves_them() {
        assert_eq!(
            next_runtime_error(Some("previous error"), "connected", None),
            None
        );
        assert_eq!(
            next_runtime_error(Some("previous error"), "reconnecting", None),
            Some("previous error".to_string())
        );
        assert_eq!(
            next_runtime_error(None, "connected", Some("token=secret-value")),
            Some("token=[redacted]".to_string())
        );
    }

    #[test]
    fn bemfa_commands_only_accept_on_and_off_with_optional_suffix() {
        assert!(bemfa_payload_is_on(b"on"));
        assert!(bemfa_payload_is_on(b" ON#voice payload "));
        assert!(bemfa_payload_is_off(b"off#ignored"));
        assert!(!bemfa_payload_is_on(b"online"));
        assert!(!bemfa_payload_is_on(b"{\"on\":true}"));
        assert_eq!(bemfa_payload_command(&[0xff]), None);
    }

    #[test]
    fn blinker_envelope_requires_owner_and_parses_component_commands() {
        assert_eq!(
            parse_blinker_command(br#"{"fromDevice":"owner","data":{"switch":"on"}}"#, "owner"),
            Some(BlinkerCommand::SwitchOn)
        );
        assert_eq!(
            parse_blinker_command(
                br#"{"fromDevice":"owner","data":"{\"get\":\"state\"}"}"#,
                "owner"
            ),
            Some(BlinkerCommand::GetState)
        );
        assert_eq!(
            parse_blinker_command(
                br#"{"fromDevice":"attacker","data":{"switch":"on"}}"#,
                "owner"
            ),
            None
        );
        assert_eq!(
            parse_blinker_command(
                br#"{"fromDevice":"owner","data":{"switch":"off"}}"#,
                "owner"
            ),
            Some(BlinkerCommand::SwitchOff)
        );
    }

    #[test]
    fn auth_refresh_is_throttled_for_rotating_tokens() {
        let now = Instant::now();
        assert!(!should_refresh_auth(Some(now), now, true));
        assert!(should_refresh_auth(
            Some(now - BLINKER_AUTH_REFRESH_MIN_INTERVAL),
            now,
            true
        ));
        assert!(!should_refresh_auth(None, now, false));
    }

    #[test]
    fn parses_official_blinker_auth_shape_and_rejects_business_errors() {
        let auth = parse_blinker_auth_response(json!({
            "message": 1000,
            "detail": {
                "broker": "blinker",
                "deviceName": "device-name",
                "host": "mqtts://broker.example.com",
                "port": "8883",
                "iotId": "mqtt-user",
                "iotToken": "temporary-token",
                "uuid": "owner-uuid"
            }
        }))
        .unwrap();
        assert_eq!(auth.device_name, "device-name");
        assert_eq!(auth.host, "broker.example.com");
        assert_eq!(auth.port, 8883);
        assert!(
            parse_blinker_auth_response(json!({
                "message": 1000,
                "detail": {
                    "broker": "blinker",
                    "deviceName": "device-name",
                    "host": "mqtt://broker.example.com",
                    "port": 1883,
                    "iotId": "mqtt-user",
                    "iotToken": "temporary-token",
                    "uuid": "owner-uuid"
                }
            }))
            .is_err()
        );
        assert!(
            parse_blinker_auth_response(json!({
                "message": 1000,
                "detail": {
                    "broker": "aliyun",
                    "deviceName": "device-name",
                    "host": "mqtts://broker.example.com",
                    "port": 8883,
                    "iotId": "mqtt-user",
                    "iotToken": "temporary-token",
                    "uuid": "owner-uuid"
                }
            }))
            .is_err()
        );
        assert!(parse_blinker_auth_response(json!({ "message": 1001 })).is_err());
    }

    #[test]
    fn blinker_heartbeat_requires_business_success() {
        assert!(parse_blinker_heartbeat_response(&json!({ "message": 1000 })).is_ok());
        assert!(parse_blinker_heartbeat_response(&json!({ "message": "1000" })).is_ok());
        assert_eq!(
            parse_blinker_heartbeat_response(&json!({ "message": 1001 })),
            Err("Blinker heartbeat was rejected".to_string())
        );
        assert!(parse_blinker_heartbeat_response(&json!({})).is_err());
    }

    #[test]
    fn blinker_state_reply_uses_the_button_widget_shape() {
        let auth = BlinkerAuth {
            broker: "blinker".to_string(),
            device_name: "device-name".to_string(),
            host: "broker.example.com".to_string(),
            port: 8883,
            iot_id: "mqtt-user".to_string(),
            iot_token: "temporary-token".to_string(),
            uuid: "owner-uuid".to_string(),
        };
        let reply = build_blinker_state_reply(&auth, Some("on"));
        assert_eq!(reply["data"]["state"], "online");
        assert_eq!(reply["data"]["switch"], json!({ "swi": "on" }));
        assert_eq!(reply["fromDevice"], "device-name");
        assert_eq!(reply["toDevice"], "owner-uuid");

        let reply_without_component = build_blinker_state_reply(&auth, None);
        assert!(reply_without_component["data"].get("switch").is_none());
    }

    #[test]
    fn successful_sessions_reset_exponential_backoff() {
        assert_eq!(next_retry_attempt(6, true), 1);
        assert_eq!(next_retry_attempt(1, false), 2);
        assert_eq!(next_retry_attempt(u32::MAX, false), u32::MAX);
    }

    #[test]
    fn blinker_reply_rate_limit_enforces_one_second_spacing() {
        let now = Instant::now();
        assert_eq!(blinker_reply_delay(None, now), Duration::ZERO);
        assert_eq!(
            blinker_reply_delay(Some(now - Duration::from_millis(250)), now),
            Duration::from_millis(750)
        );
        assert_eq!(
            blinker_reply_delay(Some(now - Duration::from_secs(2)), now),
            Duration::ZERO
        );
        assert_eq!(BLINKER_HEARTBEAT_INTERVAL, Duration::from_secs(599));
    }

    #[tokio::test]
    async fn self_signed_tls_requires_explicit_skip_verification() {
        use rumqttc::TlsConfiguration;
        use rumqttc::tokio_rustls::rustls::ServerConfig;
        use rumqttc::tokio_rustls::rustls::pki_types::{PrivateKeyDer, ServerName};
        use rumqttc::tokio_rustls::{TlsAcceptor, TlsConnector};

        ensure_rustls_crypto_provider();
        let generated = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let certificate = generated.cert.der().clone();
        let private_key = PrivateKeyDer::Pkcs8(generated.signing_key.serialize_der().into());
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], private_key)
            .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(server_config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let _ = acceptor.accept(stream).await;
            }
        });

        let connect = |skip_tls_verify| async move {
            let config = match mqtt_tls_transport(skip_tls_verify) {
                Transport::Tls(TlsConfiguration::Rustls(config)) => config,
                _ => unreachable!("WoL integrations always use rustls TLS"),
            };
            let stream = tokio::net::TcpStream::connect(address).await.unwrap();
            TlsConnector::from(config)
                .connect(ServerName::try_from("localhost").unwrap(), stream)
                .await
        };
        assert!(connect(false).await.is_err());
        assert!(connect(true).await.is_ok());
        time::timeout(Duration::from_secs(2), server)
            .await
            .expect("TLS fixture should finish")
            .unwrap();
    }
}
