use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use super::{
    ApplyCheckpoint, PanelAdapter,
    client::{PanelHttpClient, ensure_api_success, response_data, response_id},
};
use crate::panel_sync::{
    model::*,
    ownership::{deterministic_name, fingerprint},
};

pub struct SunPanelAdapter;

#[async_trait]
impl PanelAdapter for SunPanelAdapter {
    fn provider(&self) -> PanelProvider {
        PanelProvider::SunPanel
    }
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            can_create: true,
            can_update: true,
            can_update_groups: false,
            can_delete: false,
            supports_icon: true,
            residual_on_delete: true,
        }
    }
    async fn probe(&self, context: &AdapterContext) -> Result<ProbeResult, String> {
        let client = PanelHttpClient::new(&context.connection)?;
        let value = client
            .json(
                Method::POST,
                client.endpoint("version")?,
                &headers(context),
                Some(&json!({})),
                None,
            )
            .await?;
        ensure_api_success(&value)?;
        let version = response_data(&value)
            .get("version")
            .and_then(Value::as_str)
            .or_else(|| response_data(&value).as_str())
            .map(str::to_string);
        Ok(ProbeResult {
            success: true,
            provider: self.provider(),
            version,
            message: "Sun-Panel OpenAPI 验证成功".to_string(),
            capabilities: self.capabilities(),
        })
    }
    async fn inspect(
        &self,
        context: &AdapterContext,
        managed: &ManagedState,
        projection: &PanelLinkProjection,
    ) -> Result<RemoteSnapshot, String> {
        let client = PanelHttpClient::new(&context.connection)?;
        let mut snapshot = RemoteSnapshot::default();
        for (source_id, owned) in &managed.groups {
            let value = client
                .json(
                    Method::POST,
                    client.endpoint("itemGroup/getInfo")?,
                    &headers(context),
                    Some(&json!({"itemGroupID": numeric_id(&owned.remote_id)})),
                    None,
                )
                .await?;
            let data = response_data(&value);
            let exists = sun_object_exists(&value);
            let remote_fingerprint = data
                .get("title")
                .and_then(Value::as_str)
                .map(|name| {
                    fingerprint(&ProjectedGroup {
                        source_id: source_id.clone(),
                        name: name.to_string(),
                    })
                })
                .unwrap_or_default();
            snapshot.groups.insert(
                source_id.clone(),
                RemoteObject {
                    remote_id: owned.remote_id.clone(),
                    fingerprint: remote_fingerprint,
                    exists,
                },
            );
        }
        for group in &projection.groups {
            if managed.groups.contains_key(&group.source_id) {
                continue;
            }
            let only_name = group_only_name(&context.connection.id, &group.source_id);
            let value = client
                .json(
                    Method::POST,
                    client.endpoint("itemGroup/getInfo")?,
                    &headers(context),
                    Some(&json!({"onlyName": only_name})),
                    None,
                )
                .await?;
            let data = response_data(&value);
            if sun_object_exists(&value)
                && let Some(remote_id) = response_id(&value)
            {
                let remote_title = data
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let remote_group = ProjectedGroup {
                    source_id: group.source_id.clone(),
                    name: remote_title.clone(),
                };
                snapshot.groups.insert(
                    group.source_id.clone(),
                    RemoteObject {
                        remote_id: remote_id.clone(),
                        fingerprint: fingerprint(&remote_group),
                        exists: true,
                    },
                );
                snapshot.recovered.groups.insert(
                    group.source_id.clone(),
                    ManagedObject {
                        remote_id,
                        remote_group_id: None,
                        fingerprint: fingerprint(&remote_group),
                        title: remote_title,
                    },
                );
            }
        }
        for link in &projection.links {
            let only_name = managed
                .links
                .get(&link.sync_id)
                .map(|item| item.remote_id.clone())
                .unwrap_or_else(|| deterministic_name(&context.connection.id, &link.sync_id));
            let value = client
                .json(
                    Method::POST,
                    client.endpoint("item/getInfoByOnlyName")?,
                    &headers(context),
                    Some(&json!({"onlyName": only_name})),
                    None,
                )
                .await?;
            let data = response_data(&value);
            let exists = sun_object_exists(&value);
            if let Some(owned) = managed.links.get(&link.sync_id) {
                let remote_group_id = data
                    .get("itemGroupID")
                    .or_else(|| data.get("groupId"))
                    .or_else(|| data.get("group_id"))
                    .and_then(|value| {
                        value
                            .as_str()
                            .map(str::to_string)
                            .or_else(|| value.as_i64().map(|value| value.to_string()))
                    });
                let group_source_id = managed
                    .groups
                    .iter()
                    .find(|(_, group)| remote_group_id.as_deref() == Some(group.remote_id.as_str()))
                    .map(|(id, _)| id.clone())
                    .unwrap_or_default();
                let icon = sun_remote_icon(data).or_else(|| {
                    // Sun-Panel 1.8.x persists `iconUrl` in `icon_json.src`, but
                    // getInfoByOnlyName returns an empty `iconUrl`. Only trust
                    // the last applied icon while the source projection still
                    // matches the recorded ownership fingerprint. A source-side
                    // icon change must therefore produce one update before this
                    // fallback becomes eligible again.
                    (owned.fingerprint == fingerprint(link))
                        .then(|| link.icon.clone())
                        .flatten()
                });
                let remote_fingerprint = if exists {
                    fingerprint(&ProjectedLink {
                        sync_id: link.sync_id.clone(),
                        group_source_id,
                        title: data
                            .get("title")
                            .or_else(|| data.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        url: data
                            .get("url")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        icon,
                    })
                } else {
                    String::new()
                };
                snapshot.links.insert(
                    link.sync_id.clone(),
                    RemoteObject {
                        remote_id: owned.remote_id.clone(),
                        fingerprint: remote_fingerprint,
                        exists,
                    },
                );
            } else if exists {
                let remote_group_id = data.get("itemGroupID").and_then(|value| {
                    value
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| value.as_i64().map(|value| value.to_string()))
                });
                let group_source_id = managed
                    .groups
                    .iter()
                    .chain(snapshot.recovered.groups.iter())
                    .find(|(_, group)| remote_group_id.as_deref() == Some(group.remote_id.as_str()))
                    .map(|(id, _)| id.clone())
                    .unwrap_or_default();
                let remote_link = ProjectedLink {
                    sync_id: link.sync_id.clone(),
                    group_source_id,
                    title: data
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    url: data
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    icon: sun_remote_icon(data),
                };
                snapshot.links.insert(
                    link.sync_id.clone(),
                    RemoteObject {
                        remote_id: only_name.clone(),
                        fingerprint: fingerprint(&remote_link),
                        exists: true,
                    },
                );
                snapshot.recovered.links.insert(
                    link.sync_id.clone(),
                    ManagedObject {
                        remote_id: only_name,
                        remote_group_id,
                        fingerprint: fingerprint(&remote_link),
                        title: remote_link.title,
                    },
                );
            }
        }
        Ok(snapshot)
    }
    async fn apply(
        &self,
        context: &AdapterContext,
        plan: &AdapterPlan,
        checkpoint: &ApplyCheckpoint,
    ) -> Result<ManagedState, String> {
        let client = PanelHttpClient::new(&context.connection)?;
        let mut managed = plan.managed.clone();
        for group in &plan.projection.groups {
            let Some(kind) = write_kind(plan, "group", &group.source_id) else {
                continue;
            };
            debug_assert_eq!(kind, PlanActionKind::Create);
            let expected = fingerprint(group);
            let only_name = group_only_name(&context.connection.id, &group.source_id);
            let value = client
                .json(
                    Method::POST,
                    client.endpoint("itemGroup/create")?,
                    &headers(context),
                    Some(&json!({"title": group.name, "onlyName": only_name})),
                    None,
                )
                .await?;
            ensure_api_success(&value)?;
            let created = client
                .json(
                    Method::POST,
                    client.endpoint("itemGroup/getInfo")?,
                    &headers(context),
                    Some(&json!({"onlyName": only_name})),
                    None,
                )
                .await?;
            ensure_api_success(&created)?;
            let remote_id =
                response_id(&created).ok_or_else(|| "Sun-Panel 未返回分类 ID".to_string())?;
            managed.groups.insert(
                group.source_id.clone(),
                ManagedObject {
                    remote_id,
                    remote_group_id: None,
                    fingerprint: expected,
                    title: group.name.clone(),
                },
            );
            checkpoint.record(&managed);
        }
        for link in &plan.projection.links {
            let Some(kind) = write_kind(plan, "link", &link.sync_id) else {
                continue;
            };
            let group_id = managed
                .groups
                .get(&link.group_source_id)
                .map(|item| item.remote_id.clone())
                .ok_or_else(|| "Sun-Panel 分类所有权状态缺失".to_string())?;
            let only_name = managed
                .links
                .get(&link.sync_id)
                .map(|item| item.remote_id.clone())
                .unwrap_or_else(|| deterministic_name(&context.connection.id, &link.sync_id));
            let body = json!({
                "onlyName": only_name,
                "itemGroupID": numeric_id(&group_id),
                "title": link.title,
                "url": link.url,
                "iconUrl": link.icon,
            });
            let endpoint = if kind == PlanActionKind::Update {
                "item/update"
            } else {
                "item/create"
            };
            let value = client
                .json(
                    Method::POST,
                    client.endpoint(endpoint)?,
                    &headers(context),
                    Some(&body),
                    None,
                )
                .await?;
            ensure_api_success(&value)?;
            managed.links.insert(
                link.sync_id.clone(),
                ManagedObject {
                    remote_id: only_name,
                    remote_group_id: Some(group_id),
                    fingerprint: fingerprint(link),
                    title: link.title.clone(),
                },
            );
            checkpoint.record(&managed);
        }
        Ok(managed)
    }
}

fn group_only_name(connection_id: &str, source_id: &str) -> String {
    deterministic_name(connection_id, &format!("group:{source_id}"))
}

fn numeric_id(value: &str) -> Value {
    value
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(value.to_string()))
}

fn sun_object_exists(value: &Value) -> bool {
    value.get("code").and_then(Value::as_i64) == Some(0) && response_data(value).is_object()
}

fn sun_remote_icon(data: &Value) -> Option<String> {
    ["iconUrl", "icon", "icon_url"]
        .into_iter()
        .filter_map(|key| data.get(key))
        .find_map(non_empty_string)
        .or_else(|| {
            ["iconJson", "icon_json"]
                .into_iter()
                .filter_map(|key| data.get(key))
                .find_map(|value| match value {
                    Value::Object(icon) => icon.get("src").and_then(non_empty_string),
                    Value::String(icon) => serde_json::from_str::<Value>(icon)
                        .ok()
                        .and_then(|icon| icon.get("src").and_then(non_empty_string)),
                    _ => None,
                })
        })
}

fn non_empty_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn headers(context: &AdapterContext) -> Vec<(String, String)> {
    vec![("token".to_string(), context.credential.clone())]
}

fn write_kind(plan: &AdapterPlan, object_type: &str, source_id: &str) -> Option<PlanActionKind> {
    plan.preview.actions.iter().find_map(|action| {
        (action.object_type == object_type
            && action.source_id.as_deref() == Some(source_id)
            && matches!(action.kind, PlanActionKind::Create | PlanActionKind::Update))
        .then_some(action.kind)
    })
}
