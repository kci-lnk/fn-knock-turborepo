use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use super::{
    ApplyCheckpoint, PanelAdapter,
    client::{PanelHttpClient, ensure_api_success, response_data, response_id},
    collect_remote_objects, conflict, remote_string,
};
use crate::panel_sync::{
    model::*,
    ownership::{deterministic_name, fingerprint},
};

pub struct VanNavAdapter;

#[async_trait]
impl PanelAdapter for VanNavAdapter {
    fn provider(&self) -> PanelProvider {
        PanelProvider::VanNav
    }
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            can_create: true,
            can_update: true,
            can_update_groups: true,
            can_delete: true,
            supports_icon: true,
            residual_on_delete: false,
        }
    }
    async fn probe(&self, context: &AdapterContext) -> Result<ProbeResult, String> {
        let client = PanelHttpClient::new(&context.connection)?;
        let value = client
            .json(
                Method::GET,
                client.endpoint("admin/all")?,
                &headers(context),
                None,
                None,
            )
            .await?;
        ensure_api_success(&value)?;
        let version = response_data(&value)
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string);
        Ok(ProbeResult {
            success: true,
            provider: self.provider(),
            version,
            message: "Van Nav OpenAPI 验证成功".to_string(),
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
        let value = client
            .json(
                Method::GET,
                client.endpoint("admin/all")?,
                &headers(context),
                None,
                None,
            )
            .await?;
        ensure_api_success(&value)?;
        let objects = collect_remote_objects(response_data(&value));
        let mut snapshot = RemoteSnapshot::default();
        for (source_id, owned) in &managed.groups {
            let found = objects.iter().find(|object| {
                remote_string(object, &["id", "ID", "categoryId", "category_id"]).as_deref()
                    == Some(owned.remote_id.as_str())
                    && remote_string(object, &["name", "title"]).is_some()
            });
            let remote_fingerprint = found
                .and_then(|object| remote_string(object, &["name", "title"]))
                .map(|name| {
                    fingerprint(&ProjectedGroup {
                        source_id: source_id.clone(),
                        name,
                    })
                })
                .unwrap_or_default();
            snapshot.groups.insert(
                source_id.clone(),
                RemoteObject {
                    remote_id: owned.remote_id.clone(),
                    fingerprint: remote_fingerprint,
                    exists: found.is_some(),
                },
            );
        }
        for (source_id, owned) in &managed.links {
            let found = objects.iter().find(|object| {
                remote_string(object, &["id", "ID", "toolId", "tool_id"]).as_deref()
                    == Some(owned.remote_id.as_str())
                    && remote_string(object, &["url", "href"]).is_some()
            });
            let remote_fingerprint = found
                .map(|object| {
                    let remote_group_id =
                        remote_string(object, &["catelogId", "categoryId", "category_id", "fid"]);
                    let group_source_id = managed
                        .groups
                        .iter()
                        .find(|(_, group)| {
                            remote_group_id.as_deref() == Some(group.remote_id.as_str())
                        })
                        .map(|(id, _)| id.clone())
                        .unwrap_or_default();
                    fingerprint(&ProjectedLink {
                        sync_id: source_id.clone(),
                        group_source_id,
                        title: remote_string(object, &["name", "title"]).unwrap_or_default(),
                        url: remote_string(object, &["url", "href"]).unwrap_or_default(),
                        icon: remote_string(object, &["logo", "icon", "iconUrl", "icon_url"]),
                    })
                })
                .unwrap_or_default();
            snapshot.links.insert(
                source_id.clone(),
                RemoteObject {
                    remote_id: owned.remote_id.clone(),
                    fingerprint: remote_fingerprint,
                    exists: found.is_some(),
                },
            );
        }
        for group in &projection.groups {
            if managed.groups.contains_key(&group.source_id) {
                continue;
            }
            let marker = ownership_marker(&context.connection.id, "group", &group.source_id);
            if let Some(object) = objects.iter().find(|object| {
                remote_string(object, &["desc", "description"]).as_deref() == Some(marker.as_str())
                    && remote_string(object, &["url", "href"]).is_none()
            }) && let Some(remote_id) = remote_string(
                object,
                &["id", "ID", "catelogId", "categoryId", "category_id"],
            ) {
                let remote_group = ProjectedGroup {
                    source_id: group.source_id.clone(),
                    name: remote_string(object, &["name", "title"]).unwrap_or_default(),
                };
                let remote_fingerprint = fingerprint(&remote_group);
                snapshot.groups.insert(
                    group.source_id.clone(),
                    RemoteObject {
                        remote_id: remote_id.clone(),
                        fingerprint: remote_fingerprint.clone(),
                        exists: true,
                    },
                );
                snapshot.recovered.groups.insert(
                    group.source_id.clone(),
                    ManagedObject {
                        remote_id,
                        remote_group_id: None,
                        fingerprint: remote_fingerprint,
                        title: remote_group.name,
                    },
                );
            }
        }
        for link in &projection.links {
            if managed.links.contains_key(&link.sync_id) {
                continue;
            }
            let marker = ownership_marker(&context.connection.id, "link", &link.sync_id);
            if let Some(object) = objects.iter().find(|object| {
                remote_string(object, &["desc", "description"]).as_deref() == Some(marker.as_str())
                    && remote_string(object, &["url", "href"]).is_some()
            }) && let Some(remote_id) = remote_string(object, &["id", "ID", "toolId", "tool_id"])
            {
                let remote_group_id =
                    remote_string(object, &["catelogId", "categoryId", "category_id", "fid"]);
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
                    title: remote_string(object, &["name", "title"]).unwrap_or_default(),
                    url: remote_string(object, &["url", "href"]).unwrap_or_default(),
                    icon: remote_string(object, &["logo", "icon", "iconUrl", "icon_url"]),
                };
                let remote_fingerprint = fingerprint(&remote_link);
                snapshot.links.insert(
                    link.sync_id.clone(),
                    RemoteObject {
                        remote_id: remote_id.clone(),
                        fingerprint: remote_fingerprint.clone(),
                        exists: true,
                    },
                );
                snapshot.recovered.links.insert(
                    link.sync_id.clone(),
                    ManagedObject {
                        remote_id,
                        remote_group_id,
                        fingerprint: remote_fingerprint,
                        title: remote_link.title,
                    },
                );
            }
        }
        let owned_group_ids = managed
            .groups
            .values()
            .chain(snapshot.recovered.groups.values())
            .map(|item| item.remote_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        for group in &projection.groups {
            if managed.groups.contains_key(&group.source_id) {
                continue;
            }
            if let Some(object) = objects.iter().find(|object| {
                remote_string(object, &["name", "title"]).as_deref() == Some(group.name.as_str())
                    && remote_string(object, &["url", "href"]).is_none()
                    && remote_string(object, &["id", "ID", "categoryId", "category_id"])
                        .is_some_and(|id| !owned_group_ids.contains(id.as_str()))
            }) {
                snapshot.conflicts.push(conflict(
                    "group",
                    &group.source_id,
                    remote_string(object, &["id", "ID", "categoryId", "category_id"]),
                    &group.name,
                ));
            }
        }
        let owned_link_ids = managed
            .links
            .values()
            .chain(snapshot.recovered.links.values())
            .map(|item| item.remote_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        for link in &projection.links {
            if managed.links.contains_key(&link.sync_id) {
                continue;
            }
            if let Some(object) = objects.iter().find(|object| {
                remote_string(object, &["name", "title"]).as_deref() == Some(link.title.as_str())
                    && remote_string(object, &["url", "href"]).is_some()
                    && remote_string(object, &["id", "ID", "toolId", "tool_id"])
                        .is_some_and(|id| !owned_link_ids.contains(id.as_str()))
            }) {
                snapshot.conflicts.push(conflict(
                    "link",
                    &link.sync_id,
                    remote_string(object, &["id", "ID", "toolId", "tool_id"]),
                    &link.title,
                ));
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
            let existing = managed.groups.get(&group.source_id).cloned();
            let (method, endpoint) = if kind == PlanActionKind::Update
                && let Some(existing) = &existing
            {
                (Method::PUT, format!("admin/catelog/{}", existing.remote_id))
            } else {
                (Method::POST, "admin/catelog".to_string())
            };
            let marker = ownership_marker(&context.connection.id, "group", &group.source_id);
            let body = if kind == PlanActionKind::Update {
                json!({"id": numeric_id(existing.as_ref().map(|item| item.remote_id.as_str()).unwrap_or_default()), "name": group.name, "desc": marker})
            } else {
                json!({"name": group.name, "desc": marker})
            };
            let url = client.endpoint(&endpoint)?;
            let value = if method == Method::POST {
                client
                    .json_once(method, url, &headers(context), Some(&body), None)
                    .await?
            } else {
                client
                    .json(method, url, &headers(context), Some(&body), None)
                    .await?
            };
            ensure_api_success(&value)?;
            let response_remote_id = (kind == PlanActionKind::Update)
                .then(|| existing.clone())
                .flatten()
                .map(|item| item.remote_id)
                .or_else(|| response_id(&value));
            let remote_id = if let Some(remote_id) = response_remote_id {
                remote_id
            } else {
                discover_marker_id(&client, context, &marker, false).await?
            };
            managed.groups.insert(
                group.source_id.clone(),
                ManagedObject {
                    remote_id,
                    remote_group_id: None,
                    fingerprint: fingerprint(group),
                    title: group.name.clone(),
                },
            );
            checkpoint.record(&managed);
        }
        for link in &plan.projection.links {
            let Some(kind) = write_kind(plan, "link", &link.sync_id) else {
                continue;
            };
            let category_id = managed
                .groups
                .get(&link.group_source_id)
                .map(|item| item.remote_id.clone())
                .ok_or_else(|| "Van Nav 分类所有权状态缺失".to_string())?;
            let existing = managed.links.get(&link.sync_id).cloned();
            let (method, endpoint) = if kind == PlanActionKind::Update
                && let Some(existing) = &existing
            {
                (Method::PUT, format!("admin/tool/{}", existing.remote_id))
            } else {
                (Method::POST, "admin/tool".to_string())
            };
            let mut body = json!({
                "catelogId": numeric_id(&category_id),
                "name": link.title,
                "url": link.url,
                "logo": link.icon,
                "desc": ownership_marker(&context.connection.id, "link", &link.sync_id),
            });
            if kind == PlanActionKind::Update {
                body["id"] = numeric_id(
                    existing
                        .as_ref()
                        .map(|item| item.remote_id.as_str())
                        .unwrap_or_default(),
                );
            }
            let url = client.endpoint(&endpoint)?;
            let value = if method == Method::POST {
                client
                    .json_once(method, url, &headers(context), Some(&body), None)
                    .await?
            } else {
                client
                    .json(method, url, &headers(context), Some(&body), None)
                    .await?
            };
            ensure_api_success(&value)?;
            let response_remote_id = (kind == PlanActionKind::Update)
                .then(|| existing.clone())
                .flatten()
                .map(|item| item.remote_id)
                .or_else(|| response_id(&value));
            let remote_id = if let Some(remote_id) = response_remote_id {
                remote_id
            } else {
                discover_marker_id(
                    &client,
                    context,
                    &ownership_marker(&context.connection.id, "link", &link.sync_id),
                    true,
                )
                .await?
            };
            managed.links.insert(
                link.sync_id.clone(),
                ManagedObject {
                    remote_id,
                    remote_group_id: Some(category_id),
                    fingerprint: fingerprint(link),
                    title: link.title.clone(),
                },
            );
            checkpoint.record(&managed);
        }
        let live_links = plan
            .projection
            .links
            .iter()
            .map(|item| item.sync_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let stale_links = managed
            .links
            .iter()
            .filter(|(id, _)| !live_links.contains(id.as_str()))
            .map(|(id, item)| (id.clone(), item.remote_id.clone()))
            .collect::<Vec<_>>();
        for (id, remote_id) in stale_links {
            if action_is(plan, PlanActionKind::Delete, "link", &id) {
                let value = client
                    .json(
                        Method::DELETE,
                        client.endpoint(&format!("admin/tool/{remote_id}"))?,
                        &headers(context),
                        None,
                        None,
                    )
                    .await?;
                ensure_api_success(&value)?;
            }
            managed.links.remove(&id);
            checkpoint.record(&managed);
        }
        let live_groups = plan
            .projection
            .groups
            .iter()
            .map(|item| item.source_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let stale_groups = managed
            .groups
            .iter()
            .filter(|(id, _)| !live_groups.contains(id.as_str()))
            .map(|(id, item)| (id.clone(), item.remote_id.clone()))
            .collect::<Vec<_>>();
        for (id, remote_id) in stale_groups {
            if action_is(plan, PlanActionKind::Delete, "group", &id) {
                let value = client
                    .json(
                        Method::DELETE,
                        client.endpoint(&format!("admin/catelog/{remote_id}"))?,
                        &headers(context),
                        None,
                        None,
                    )
                    .await?;
                ensure_api_success(&value)?;
            }
            managed.groups.remove(&id);
            checkpoint.record(&managed);
        }
        Ok(managed)
    }
}

fn numeric_id(value: &str) -> Value {
    value
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(value.to_string()))
}

fn ownership_marker(connection_id: &str, object_type: &str, source_id: &str) -> String {
    deterministic_name(connection_id, &format!("van-nav:{object_type}:{source_id}"))
}

async fn discover_marker_id(
    client: &PanelHttpClient,
    context: &AdapterContext,
    marker: &str,
    link: bool,
) -> Result<String, String> {
    let value = client
        .json(
            Method::GET,
            client.endpoint("admin/all")?,
            &headers(context),
            None,
            None,
        )
        .await?;
    ensure_api_success(&value)?;
    collect_remote_objects(response_data(&value))
        .into_iter()
        .find(|object| {
            remote_string(object, &["desc", "description"]).as_deref() == Some(marker)
                && remote_string(object, &["url", "href"]).is_some() == link
        })
        .and_then(|object| {
            if link {
                remote_string(object, &["id", "ID", "toolId", "tool_id"])
            } else {
                remote_string(
                    object,
                    &["id", "ID", "catelogId", "categoryId", "category_id"],
                )
            }
        })
        .ok_or_else(|| "Van Nav 创建成功后无法确认远端对象 ID".to_string())
}

fn headers(context: &AdapterContext) -> Vec<(String, String)> {
    vec![(
        "authorization".to_string(),
        format!("Bearer {}", context.credential),
    )]
}

fn write_kind(plan: &AdapterPlan, object_type: &str, source_id: &str) -> Option<PlanActionKind> {
    plan.preview.actions.iter().find_map(|action| {
        (action.object_type == object_type
            && action.source_id.as_deref() == Some(source_id)
            && matches!(action.kind, PlanActionKind::Create | PlanActionKind::Update))
        .then_some(action.kind)
    })
}

fn action_is(plan: &AdapterPlan, kind: PlanActionKind, object_type: &str, source_id: &str) -> bool {
    plan.preview.actions.iter().any(|action| {
        action.kind == kind
            && action.object_type == object_type
            && action.source_id.as_deref() == Some(source_id)
    })
}
