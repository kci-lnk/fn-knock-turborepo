use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Map, Value};

use super::{
    ApplyCheckpoint, PanelAdapter,
    client::{PanelHttpClient, ensure_api_success, response_id},
    collect_remote_objects, conflict, remote_string,
};
use crate::panel_sync::{
    model::*,
    ownership::{deterministic_name, fingerprint},
};

pub struct OneNavAdapter;

#[async_trait]
impl PanelAdapter for OneNavAdapter {
    fn provider(&self) -> PanelProvider {
        PanelProvider::OneNav
    }
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            can_create: true,
            can_update: true,
            can_update_groups: true,
            can_delete: true,
            supports_icon: false,
            residual_on_delete: false,
        }
    }
    async fn probe(&self, context: &AdapterContext) -> Result<ProbeResult, String> {
        let value = call(context, "category_list", vec![]).await?;
        ensure_api_success(&value)?;
        Ok(ProbeResult {
            success: true,
            provider: self.provider(),
            version: None,
            message: "OneNav API 验证成功".to_string(),
            capabilities: self.capabilities(),
        })
    }
    async fn inspect(
        &self,
        context: &AdapterContext,
        managed: &ManagedState,
        projection: &PanelLinkProjection,
    ) -> Result<RemoteSnapshot, String> {
        let category_objects = list_objects(context, "category_list").await?;
        let link_objects = list_objects(context, "link_list").await?;
        let mut snapshot = RemoteSnapshot::default();
        for (source_id, owned) in &managed.groups {
            let found = category_objects.iter().find(|object| {
                remote_string(object, &["id", "ID", "category_id"]).as_deref()
                    == Some(&owned.remote_id)
            });
            let fingerprint = found
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
                    fingerprint,
                    exists: found.is_some(),
                },
            );
        }
        for (source_id, owned) in &managed.links {
            let found = link_objects.iter().find(|object| {
                remote_string(object, &["id", "ID", "link_id"]).as_deref() == Some(&owned.remote_id)
            });
            let fingerprint = found
                .map(|object| {
                    let group_source_id = managed
                        .groups
                        .iter()
                        .find(|(_, group)| {
                            remote_string(object, &["fid", "category_id"]).as_deref()
                                == Some(group.remote_id.as_str())
                        })
                        .map(|(id, _)| id.clone())
                        .unwrap_or_default();
                    fingerprint(&ProjectedLink {
                        sync_id: source_id.clone(),
                        group_source_id,
                        title: remote_string(object, &["title", "name"]).unwrap_or_default(),
                        url: remote_string(object, &["url"]).unwrap_or_default(),
                        icon: None,
                    })
                })
                .unwrap_or_default();
            snapshot.links.insert(
                source_id.clone(),
                RemoteObject {
                    remote_id: owned.remote_id.clone(),
                    fingerprint,
                    exists: found.is_some(),
                },
            );
        }
        for group in &projection.groups {
            if managed.groups.contains_key(&group.source_id) {
                continue;
            }
            let marker = ownership_marker(&context.connection.id, "group", &group.source_id);
            if let Some(object) = category_objects.iter().find(|object| {
                remote_string(object, &["description", "desc"]).as_deref() == Some(marker.as_str())
            }) && let Some(remote_id) = remote_string(object, &["id", "ID", "category_id"])
            {
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
            if let Some(object) = link_objects.iter().find(|object| {
                remote_string(object, &["description", "desc"]).as_deref() == Some(marker.as_str())
            }) && let Some(remote_id) = remote_string(object, &["id", "ID", "link_id"])
            {
                let remote_group_id = remote_string(object, &["fid", "category_id"]);
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
                    title: remote_string(object, &["title", "name"]).unwrap_or_default(),
                    url: remote_string(object, &["url"]).unwrap_or_default(),
                    icon: None,
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
            if let Some(object) = category_objects.iter().find(|object| {
                remote_string(object, &["name", "title"]).as_deref() == Some(group.name.as_str())
                    && remote_string(object, &["id", "ID", "category_id"])
                        .is_some_and(|id| !owned_group_ids.contains(id.as_str()))
            }) {
                snapshot.conflicts.push(conflict(
                    "group",
                    &group.source_id,
                    remote_string(object, &["id", "ID", "category_id"]),
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
            if let Some(object) = link_objects.iter().find(|object| {
                remote_string(object, &["title", "name"]).as_deref() == Some(link.title.as_str())
                    && remote_string(object, &["id", "ID", "link_id"])
                        .is_some_and(|id| !owned_link_ids.contains(id.as_str()))
            }) {
                snapshot.conflicts.push(conflict(
                    "link",
                    &link.sync_id,
                    remote_string(object, &["id", "ID", "link_id"]),
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
        let mut managed = plan.managed.clone();
        for group in &plan.projection.groups {
            let Some(kind) = write_kind(plan, "group", &group.source_id) else {
                continue;
            };
            let existing = managed.groups.get(&group.source_id).cloned();
            let marker = ownership_marker(&context.connection.id, "group", &group.source_id);
            let mut fields = vec![
                ("name".to_string(), group.name.clone()),
                ("property".to_string(), "0".to_string()),
                ("weight".to_string(), "0".to_string()),
                ("description".to_string(), marker.clone()),
                ("font_icon".to_string(), "fa fa-folder".to_string()),
                ("fid".to_string(), "0".to_string()),
            ];
            let method = if kind == PlanActionKind::Update
                && let Some(existing) = &existing
            {
                fields.push(("id".to_string(), existing.remote_id.clone()));
                "edit_category"
            } else {
                "add_category"
            };
            let value = call(context, method, fields).await?;
            ensure_api_success(&value)?;
            let response_remote_id = (kind == PlanActionKind::Update)
                .then(|| existing.clone())
                .flatten()
                .map(|item| item.remote_id)
                .or_else(|| response_id(&value));
            let remote_id = if let Some(remote_id) = response_remote_id {
                remote_id
            } else {
                discover_marker_id(context, "category_list", &marker, false).await?
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
            let group_id = managed
                .groups
                .get(&link.group_source_id)
                .map(|item| item.remote_id.clone())
                .ok_or_else(|| "OneNav 分类所有权状态缺失".to_string())?;
            let existing = managed.links.get(&link.sync_id).cloned();
            let mut fields = vec![
                ("fid".to_string(), group_id.clone()),
                ("title".to_string(), link.title.clone()),
                ("url".to_string(), link.url.clone()),
                ("url_standby".to_string(), String::new()),
                ("property".to_string(), "0".to_string()),
                ("weight".to_string(), "0".to_string()),
                (
                    "description".to_string(),
                    ownership_marker(&context.connection.id, "link", &link.sync_id),
                ),
            ];
            let method = if kind == PlanActionKind::Update
                && let Some(existing) = &existing
            {
                fields.push(("id".to_string(), existing.remote_id.clone()));
                "edit_link"
            } else {
                "add_link"
            };
            let value = call(context, method, fields).await?;
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
                    context,
                    "link_list",
                    &ownership_marker(&context.connection.id, "link", &link.sync_id),
                    true,
                )
                .await?
            };
            managed.links.insert(
                link.sync_id.clone(),
                ManagedObject {
                    remote_id,
                    remote_group_id: Some(group_id),
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
                let value = call(context, "del_link", vec![("id".to_string(), remote_id)]).await?;
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
                let value =
                    call(context, "del_category", vec![("id".to_string(), remote_id)]).await?;
                ensure_api_success(&value)?;
            }
            managed.groups.remove(&id);
            checkpoint.record(&managed);
        }
        Ok(managed)
    }
}

async fn call(
    context: &AdapterContext,
    method: &str,
    mut fields: Vec<(String, String)>,
) -> Result<serde_json::Value, String> {
    let client = PanelHttpClient::new(&context.connection)?;
    fields.push(("token".to_string(), context.credential.clone()));
    let pretty = !context.connection.api_path.contains("index.php");
    let mut url = client.endpoint(if pretty { method } else { "" })?;
    if !pretty {
        url.query_pairs_mut().append_pair("method", method);
    }
    if matches!(method, "add_category" | "add_link") {
        client
            .json_once(Method::POST, url, &[], None, Some(&fields))
            .await
    } else {
        client
            .json(Method::POST, url, &[], None, Some(&fields))
            .await
    }
}

async fn list_objects(
    context: &AdapterContext,
    method: &str,
) -> Result<Vec<Map<String, Value>>, String> {
    const PAGE_SIZE: usize = 200;
    const MAX_PAGES: usize = 10;
    let mut output = Vec::new();
    for page in 1..=MAX_PAGES {
        let value = call(
            context,
            method,
            vec![
                ("page".to_string(), page.to_string()),
                ("limit".to_string(), PAGE_SIZE.to_string()),
            ],
        )
        .await?;
        ensure_api_success(&value)?;
        let items = collect_remote_objects(&value);
        let item_count = items.len();
        output.extend(items.into_iter().cloned());
        if item_count < PAGE_SIZE {
            return Ok(output);
        }
    }
    Err(format!(
        "OneNav {method} 超过 {MAX_PAGES} 页安全上限，请减少远端条目后重试"
    ))
}

fn ownership_marker(connection_id: &str, object_type: &str, source_id: &str) -> String {
    deterministic_name(connection_id, &format!("one-nav:{object_type}:{source_id}"))
}

async fn discover_marker_id(
    context: &AdapterContext,
    method: &str,
    marker: &str,
    link: bool,
) -> Result<String, String> {
    list_objects(context, method)
        .await?
        .into_iter()
        .find(|object| remote_string(object, &["description", "desc"]).as_deref() == Some(marker))
        .and_then(|object| {
            if link {
                remote_string(&object, &["id", "ID", "link_id"])
            } else {
                remote_string(&object, &["id", "ID", "category_id"])
            }
        })
        .ok_or_else(|| "OneNav 创建成功后无法确认远端对象 ID".to_string())
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
