use std::collections::HashSet;

use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use super::model::*;

pub fn deterministic_name(connection_id: &str, source_id: &str) -> String {
    let digest = Sha256::digest(format!("{connection_id}:{source_id}").as_bytes());
    format!("fn-knock-{}", &hex::encode(digest)[..24])
}

pub fn fingerprint<T: serde::Serialize>(value: &T) -> String {
    hex::encode(Sha256::digest(
        serde_json::to_vec(value).unwrap_or_default(),
    ))
}

pub fn build_plan(
    connection: &PanelConnection,
    projection: PanelLinkProjection,
    mut managed: ManagedState,
    remote: RemoteSnapshot,
    capabilities: &AdapterCapabilities,
) -> AdapterPlan {
    for (source_id, object) in &remote.recovered.groups {
        managed
            .groups
            .entry(source_id.clone())
            .or_insert_with(|| object.clone());
    }
    for (source_id, object) in &remote.recovered.links {
        managed
            .links
            .entry(source_id.clone())
            .or_insert_with(|| object.clone());
    }
    let mut actions = remote.conflicts.clone();
    let projected_groups = projection
        .groups
        .iter()
        .map(|group| group.source_id.as_str())
        .collect::<HashSet<_>>();
    let projected_links = projection
        .links
        .iter()
        .map(|link| link.sync_id.as_str())
        .collect::<HashSet<_>>();
    for group in &projection.groups {
        let expected = fingerprint(group);
        match managed.groups.get(&group.source_id) {
            None => actions.push(action(
                PlanActionKind::Create,
                "group",
                &group.source_id,
                None,
                &group.name,
                "创建专属分类",
            )),
            Some(owned)
                if !remote
                    .groups
                    .get(&group.source_id)
                    .is_some_and(|item| item.exists) =>
            {
                actions.push(action(
                    PlanActionKind::Create,
                    "group",
                    &group.source_id,
                    Some(&owned.remote_id),
                    &group.name,
                    "远端分类已被删除，将重新创建",
                ))
            }
            Some(owned)
                if owned.fingerprint != expected
                    || remote.groups.get(&group.source_id).is_some_and(|item| {
                        item.fingerprint != expected || item.remote_id != owned.remote_id
                    }) =>
            {
                let (kind, detail) = if capabilities.can_update_groups {
                    (PlanActionKind::Update, "恢复 fn-knock 分类定义")
                } else {
                    (
                        PlanActionKind::Residual,
                        "面板官方 API 不支持分类改名，将保留远端分类标题",
                    )
                };
                actions.push(action(
                    kind,
                    "group",
                    &group.source_id,
                    Some(&owned.remote_id),
                    &group.name,
                    detail,
                ))
            }
            Some(owned) => actions.push(action(
                PlanActionKind::Unchanged,
                "group",
                &group.source_id,
                Some(&owned.remote_id),
                &group.name,
                "无需变更",
            )),
        }
    }
    for link in &projection.links {
        let expected = fingerprint(link);
        match managed.links.get(&link.sync_id) {
            None => actions.push(action(
                PlanActionKind::Create,
                "link",
                &link.sync_id,
                None,
                &link.title,
                "创建链接",
            )),
            Some(owned)
                if !remote
                    .links
                    .get(&link.sync_id)
                    .is_some_and(|item| item.exists) =>
            {
                actions.push(action(
                    PlanActionKind::Create,
                    "link",
                    &link.sync_id,
                    Some(&owned.remote_id),
                    &link.title,
                    "远端链接已被删除，将重新创建",
                ))
            }
            Some(owned)
                if owned.fingerprint != expected
                    || remote.links.get(&link.sync_id).is_some_and(|item| {
                        item.fingerprint != expected || item.remote_id != owned.remote_id
                    }) =>
            {
                actions.push(action(
                    PlanActionKind::Update,
                    "link",
                    &link.sync_id,
                    Some(&owned.remote_id),
                    &link.title,
                    "覆盖面板侧修改",
                ))
            }
            Some(owned) => actions.push(action(
                PlanActionKind::Unchanged,
                "link",
                &link.sync_id,
                Some(&owned.remote_id),
                &link.title,
                "无需变更",
            )),
        }
    }
    for (source_id, owned) in &managed.links {
        if !projected_links.contains(source_id.as_str()) {
            let remote_exists = remote.links.get(source_id).is_none_or(|item| item.exists);
            let kind = if capabilities.can_delete && remote_exists {
                PlanActionKind::Delete
            } else if capabilities.can_delete {
                PlanActionKind::Unchanged
            } else {
                PlanActionKind::Residual
            };
            actions.push(action(
                kind,
                "link",
                source_id,
                Some(&owned.remote_id),
                &owned.title,
                if capabilities.can_delete && remote_exists {
                    "删除失效的自有链接"
                } else if capabilities.can_delete {
                    "远端链接已不存在，仅清除本地所有权登记"
                } else {
                    "面板没有稳定删除接口，将保留远端残留"
                },
            ));
        }
    }
    for (source_id, owned) in &managed.groups {
        if !projected_groups.contains(source_id.as_str()) {
            let remote_exists = remote.groups.get(source_id).is_none_or(|item| item.exists);
            let kind = if capabilities.can_delete && remote_exists {
                PlanActionKind::Delete
            } else if capabilities.can_delete {
                PlanActionKind::Unchanged
            } else {
                PlanActionKind::Residual
            };
            actions.push(action(
                kind,
                "group",
                source_id,
                Some(&owned.remote_id),
                &owned.title,
                if capabilities.can_delete && remote_exists {
                    "删除空的自有分类"
                } else if capabilities.can_delete {
                    "远端分类已不存在，仅清除本地所有权登记"
                } else {
                    "面板没有稳定删除接口，将保留远端残留"
                },
            ));
        }
    }
    let mut counts = PlanCounts::default();
    for action in &actions {
        match action.kind {
            PlanActionKind::Create => counts.create += 1,
            PlanActionKind::Update => counts.update += 1,
            PlanActionKind::Delete => counts.delete += 1,
            PlanActionKind::Unchanged => counts.unchanged += 1,
            PlanActionKind::Residual => counts.residual += 1,
            PlanActionKind::Conflict => counts.conflict += 1,
        }
    }
    let mut warnings = projection.warnings.clone();
    warnings.extend(remote.warnings);
    if counts.residual > 0 {
        warnings
            .push("Sun-Panel 不提供稳定删除和分类改名接口，相关对象仅报告为远端残留".to_string());
    }
    let source_revision = projection.revision.clone();
    let hash_input = serde_json::json!({
        "connection_id": connection.id,
        "source_revision": source_revision,
        "actions": actions,
    });
    let plan_hash = fingerprint(&hash_input);
    let expires_at = (OffsetDateTime::now_utc() + Duration::minutes(10))
        .format(&Rfc3339)
        .unwrap_or_default();
    let can_apply = counts.conflict == 0;
    AdapterPlan {
        preview: SyncPreview {
            connection_id: connection.id.clone(),
            source_revision,
            plan_hash,
            counts,
            actions,
            warnings,
            can_apply,
            expires_at,
        },
        projection,
        managed,
    }
}

fn action(
    kind: PlanActionKind,
    object_type: &str,
    source_id: &str,
    remote_id: Option<&str>,
    title: &str,
    detail: &str,
) -> PlanAction {
    PlanAction {
        kind,
        object_type: object_type.to_string(),
        source_id: Some(source_id.to_string()),
        remote_id: remote_id.map(str::to_string),
        title: title.to_string(),
        detail: detail.to_string(),
    }
}
