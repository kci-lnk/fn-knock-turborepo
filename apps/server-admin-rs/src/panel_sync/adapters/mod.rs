use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use super::model::*;

mod client;
mod one_nav;
mod sun_panel;
mod van_nav;

/// Keeps the latest successfully applied ownership state available even when a
/// later remote operation fails or application shutdown cancels the adapter.
/// This prevents a partially created remote object from becoming an unowned
/// name conflict on the next run.
#[derive(Clone)]
pub struct ApplyCheckpoint(Arc<Mutex<ManagedState>>);

impl ApplyCheckpoint {
    pub fn new(initial: ManagedState) -> Self {
        Self(Arc::new(Mutex::new(initial)))
    }

    pub fn record(&self, managed: &ManagedState) {
        *self.0.lock().unwrap_or_else(|error| error.into_inner()) = managed.clone();
    }

    pub fn latest(&self) -> ManagedState {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

#[async_trait]
pub trait PanelAdapter: Send + Sync {
    fn provider(&self) -> PanelProvider;
    fn capabilities(&self) -> AdapterCapabilities;
    async fn probe(&self, context: &AdapterContext) -> Result<ProbeResult, String>;
    async fn inspect(
        &self,
        context: &AdapterContext,
        managed: &ManagedState,
        projection: &PanelLinkProjection,
    ) -> Result<RemoteSnapshot, String>;
    fn plan(
        &self,
        connection: &PanelConnection,
        projection: PanelLinkProjection,
        managed: ManagedState,
        remote: RemoteSnapshot,
    ) -> AdapterPlan {
        crate::panel_sync::ownership::build_plan(
            connection,
            projection,
            managed,
            remote,
            &self.capabilities(),
        )
    }
    async fn apply(
        &self,
        context: &AdapterContext,
        plan: &AdapterPlan,
        checkpoint: &ApplyCheckpoint,
    ) -> Result<ManagedState, String>;
}

pub struct AdapterRegistry;

impl AdapterRegistry {
    pub fn resolve(provider: PanelProvider) -> &'static dyn PanelAdapter {
        match provider {
            PanelProvider::SunPanel => &sun_panel::SunPanelAdapter,
            PanelProvider::OneNav => &one_nav::OneNavAdapter,
            PanelProvider::VanNav => &van_nav::VanNavAdapter,
        }
    }
}

pub fn provider_descriptors() -> Vec<ProviderDescriptor> {
    [
        PanelProvider::SunPanel,
        PanelProvider::OneNav,
        PanelProvider::VanNav,
    ]
    .into_iter()
    .map(|provider| {
        let capabilities = AdapterRegistry::resolve(provider).capabilities();
        ProviderDescriptor {
            provider,
            name: provider.label().to_string(),
            default_api_path: provider.default_api_path().to_string(),
            supports_delete: capabilities.can_delete,
            supports_icon: capabilities.supports_icon,
            notes: if capabilities.residual_on_delete {
                vec!["官方 OpenAPI 没有稳定删除和分类改名接口，相关变更会报告为残留".to_string()]
            } else {
                vec!["仅管理当前连接创建并登记的分类和链接".to_string()]
            },
        }
    })
    .collect()
}

fn collect_remote_objects(
    value: &serde_json::Value,
) -> Vec<&serde_json::Map<String, serde_json::Value>> {
    fn visit<'a>(
        value: &'a serde_json::Value,
        output: &mut Vec<&'a serde_json::Map<String, serde_json::Value>>,
    ) {
        match value {
            serde_json::Value::Array(values) => {
                values.iter().for_each(|value| visit(value, output))
            }
            serde_json::Value::Object(object) => {
                if remote_string(object, &["id", "ID", "category_id", "tool_id"]).is_some() {
                    output.push(object);
                }
                object.values().for_each(|value| visit(value, output));
            }
            _ => {}
        }
    }
    let mut output = Vec::new();
    visit(value, &mut output);
    output
}

fn remote_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_i64().map(|value| value.to_string()))
        })
}

fn conflict(
    object_type: &str,
    source_id: &str,
    remote_id: Option<String>,
    title: &str,
) -> PlanAction {
    PlanAction {
        kind: PlanActionKind::Conflict,
        object_type: object_type.to_string(),
        source_id: Some(source_id.to_string()),
        remote_id,
        title: title.to_string(),
        detail: "发现未登记的同名远端对象；为保护面板内容，fn-knock 不会接管".to_string(),
    }
}
