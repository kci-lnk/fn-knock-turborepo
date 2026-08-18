use std::collections::{BTreeMap, HashSet};

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::model::{GroupMode, GroupingConfig, PanelLinkProjection, ProjectedGroup, ProjectedLink};

pub fn eligible_mappings_missing_sync_id(config: &Value) -> usize {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|mapping| {
            mapping.get("disabled").and_then(Value::as_bool) != Some(true)
                && mapping.get("service_role").and_then(Value::as_str) == Some("app")
        })
        .filter(|mapping| {
            mapping
                .get("sync_id")
                .and_then(Value::as_str)
                .is_none_or(|value| uuid::Uuid::parse_str(value).is_err())
        })
        .count()
}

pub fn project(config: &Value, grouping: &GroupingConfig) -> PanelLinkProjection {
    let context = crate::proxy_config::public_host_link_context(config);
    let group_names = config
        .get("host_mapping_groups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| {
            let name = group.get("name")?.as_str()?.trim();
            if name.is_empty() {
                return None;
            }
            Some((group.get("id")?.as_str()?.to_string(), name.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let namespace = grouping.namespace.trim();
    let namespace = if namespace.is_empty() {
        "fn-knock"
    } else {
        namespace
    };
    let mut groups = BTreeMap::<String, ProjectedGroup>::new();
    let mut links = Vec::new();
    let warnings = Vec::new();
    for mapping in config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if mapping.get("disabled").and_then(Value::as_bool) == Some(true)
            || mapping.get("service_role").and_then(Value::as_str) != Some("app")
        {
            continue;
        }
        let Some(sync_id) = mapping.get("sync_id").and_then(Value::as_str) else {
            continue;
        };
        let Some(host) = mapping.get("host").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if host.is_empty() {
            continue;
        }
        let (group_source_id, group_name) = match grouping.mode {
            GroupMode::Single => (
                "single".to_string(),
                if grouping.single_group_name.trim().is_empty() {
                    namespace.to_string()
                } else {
                    grouping.single_group_name.trim().to_string()
                },
            ),
            GroupMode::Mirror => {
                let raw = mapping
                    .get("group_id")
                    .and_then(Value::as_str)
                    .filter(|id| group_names.contains_key(*id));
                match raw {
                    Some(id) => (id.to_string(), format!("{namespace} · {}", group_names[id])),
                    None => ("ungrouped".to_string(), namespace.to_string()),
                }
            }
        };
        groups
            .entry(group_source_id.clone())
            .or_insert(ProjectedGroup {
                source_id: group_source_id.clone(),
                name: group_name,
            });
        let Some(object) = mapping.as_object() else {
            continue;
        };
        links.push(ProjectedLink {
            sync_id: sync_id.to_string(),
            group_source_id,
            title: crate::proxy_config::resolve_public_host_title(object, host),
            url: crate::proxy_config::public_host_url(&context, host),
            icon: Some(crate::proxy_config::public_panel_icon_url(
                &context, object, host, sync_id,
            )),
        });
    }
    let mut projection = PanelLinkProjection {
        revision: String::new(),
        groups: groups.into_values().collect(),
        links,
        warnings,
    };
    projection
        .groups
        .sort_by(|a, b| a.source_id.cmp(&b.source_id));
    projection.links.sort_by(|a, b| a.sync_id.cmp(&b.sync_id));
    projection.warnings = projection
        .warnings
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    projection.warnings.sort();
    let encoded = serde_json::to_vec(&projection).unwrap_or_default();
    projection.revision = hex::encode(Sha256::digest(encoded));
    projection
}
