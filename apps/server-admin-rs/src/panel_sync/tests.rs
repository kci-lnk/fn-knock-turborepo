use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::{
    adapters::{AdapterRegistry, ApplyCheckpoint},
    credentials::CredentialStore,
    model::*,
    ownership::{build_plan, deterministic_name, fingerprint},
    projection::{eligible_mappings_missing_sync_id, project},
};

fn connection(provider: PanelProvider, base_url: String, api_path: &str) -> PanelConnection {
    PanelConnection {
        id: "11111111-1111-4111-8111-111111111111".to_string(),
        name: "test".to_string(),
        provider,
        base_url,
        api_path: api_path.to_string(),
        allow_invalid_tls: false,
        grouping: GroupingConfig::default(),
        auto_sync: AutoSyncConfig::default(),
        credential_configured: true,
        verified_at: None,
        verified_version: None,
        created_at: String::new(),
        updated_at: String::new(),
        last_run: None,
        next_sync_at: None,
    }
}

async fn mock_json_response(body: &'static str) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let read = stream.read(&mut chunk).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| index + 4);
            if let Some(header_end) = header_end {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + length {
                    break;
                }
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request).unwrap()
    });
    (format!("http://{address}"), task)
}

async fn mock_json_responses(
    bodies: Vec<String>,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for body in bodies {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                let Some(header_end) = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|index| index + 4)
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + length {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            requests.push(String::from_utf8(request).unwrap());
        }
        requests
    });
    (format!("http://{address}"), task)
}

#[test]
fn projection_filters_auth_and_disabled_mappings_and_never_exports_targets() {
    let config = json!({
        "host_mappings": [
            {"host":"app.example.test","sync_id":"11111111-1111-4111-8111-111111111111","target":"http://10.0.0.2:80","service_role":"app","disabled":false,"title":"App"},
            {"host":"off.example.test","sync_id":"22222222-2222-4222-8222-222222222222","target":"http://10.0.0.3:80","service_role":"app","disabled":true},
            {"host":"auth.example.test","sync_id":"33333333-3333-4333-8333-333333333333","target":"http://127.0.0.1:7998","service_role":"auth","disabled":false}
        ]
    });
    let projection = project(
        &config,
        &GroupingConfig {
            mode: GroupMode::Single,
            namespace: "fn-knock".to_string(),
            single_group_name: "NAS".to_string(),
        },
    );
    assert_eq!(projection.links.len(), 1);
    assert_eq!(projection.links[0].title, "App");
    assert!(projection.links[0].url.contains("app.example.test"));
    assert_eq!(
        projection.links[0].icon.as_deref(),
        Some(
            "http://app.example.test:7999/__assets__/website_icon.11111111-1111-4111-8111-111111111111.png"
        )
    );
    assert!(
        !serde_json::to_string(&projection)
            .unwrap()
            .contains("10.0.0.2")
    );
}

#[tokio::test]
async fn sun_panel_treats_an_empty_remote_icon_as_no_icon() {
    let (base_url, requests) = mock_json_responses(vec![
        json!({"code": 0, "data": {"itemGroupID": 2, "title": "NAS"}}).to_string(),
        json!({
            "code": 0,
            "data": {
                "onlyName": "link-only-name",
                "itemGroupID": 2,
                "title": "Files",
                "url": "https://files.example.test/",
                "iconUrl": ""
            }
        })
        .to_string(),
    ])
    .await;
    let context = AdapterContext {
        connection: connection(PanelProvider::SunPanel, base_url, "/openapi/v1"),
        credential: "sun-secret".to_string(),
    };
    let group = ProjectedGroup {
        source_id: "single".to_string(),
        name: "NAS".to_string(),
    };
    let link = ProjectedLink {
        sync_id: "mapping-1".to_string(),
        group_source_id: "single".to_string(),
        title: "Files".to_string(),
        url: "https://files.example.test/".to_string(),
        icon: None,
    };
    let mut managed = ManagedState::default();
    managed.groups.insert(
        group.source_id.clone(),
        ManagedObject {
            remote_id: "2".to_string(),
            fingerprint: fingerprint(&group),
            title: group.name.clone(),
            ..ManagedObject::default()
        },
    );
    managed.links.insert(
        link.sync_id.clone(),
        ManagedObject {
            remote_id: "link-only-name".to_string(),
            remote_group_id: Some("2".to_string()),
            fingerprint: fingerprint(&link),
            title: link.title.clone(),
        },
    );
    let projection = PanelLinkProjection {
        revision: "revision".to_string(),
        groups: vec![group],
        links: vec![link],
        warnings: Vec::new(),
    };
    let adapter = AdapterRegistry::resolve(PanelProvider::SunPanel);
    let remote = adapter
        .inspect(&context, &managed, &projection)
        .await
        .unwrap();
    let plan = build_plan(
        &context.connection,
        projection,
        managed,
        remote,
        &adapter.capabilities(),
    );
    assert_eq!(plan.preview.counts.update, 0);
    assert_eq!(plan.preview.counts.unchanged, 2);
    assert_eq!(requests.await.unwrap().len(), 2);
}

async fn sun_plan_with_unreadable_remote_icon(
    link: ProjectedLink,
    managed_fingerprint: String,
    remote_url: &str,
) -> AdapterPlan {
    let (base_url, requests) = mock_json_responses(vec![
        json!({"code": 0, "data": {"itemGroupID": 2, "title": "NAS"}}).to_string(),
        json!({
            "code": 0,
            "data": {
                "onlyName": "link-only-name",
                "itemGroupID": 2,
                "title": "Files",
                "url": remote_url,
                "iconUrl": ""
            }
        })
        .to_string(),
    ])
    .await;
    let context = AdapterContext {
        connection: connection(PanelProvider::SunPanel, base_url, "/openapi/v1"),
        credential: "sun-secret".to_string(),
    };
    let group = ProjectedGroup {
        source_id: "single".to_string(),
        name: "NAS".to_string(),
    };
    let mut managed = ManagedState::default();
    managed.groups.insert(
        group.source_id.clone(),
        ManagedObject {
            remote_id: "2".to_string(),
            fingerprint: fingerprint(&group),
            title: group.name.clone(),
            ..ManagedObject::default()
        },
    );
    managed.links.insert(
        link.sync_id.clone(),
        ManagedObject {
            remote_id: "link-only-name".to_string(),
            remote_group_id: Some("2".to_string()),
            fingerprint: managed_fingerprint,
            title: link.title.clone(),
        },
    );
    let projection = PanelLinkProjection {
        revision: "revision".to_string(),
        groups: vec![group],
        links: vec![link],
        warnings: Vec::new(),
    };
    let adapter = AdapterRegistry::resolve(PanelProvider::SunPanel);
    let remote = adapter
        .inspect(&context, &managed, &projection)
        .await
        .unwrap();
    let plan = build_plan(
        &context.connection,
        projection,
        managed,
        remote,
        &adapter.capabilities(),
    );
    assert_eq!(requests.await.unwrap().len(), 2);
    plan
}

#[tokio::test]
async fn sun_panel_does_not_repeat_updates_when_icon_cannot_be_read_back() {
    let link = ProjectedLink {
        sync_id: "mapping-1".to_string(),
        group_source_id: "single".to_string(),
        title: "Files".to_string(),
        url: "https://files.example.test/".to_string(),
        icon: Some("https://files.example.test/__assets__/website_icon.uuid.ico".to_string()),
    };
    let plan = sun_plan_with_unreadable_remote_icon(
        link.clone(),
        fingerprint(&link),
        "https://files.example.test/",
    )
    .await;
    assert_eq!(plan.preview.counts.update, 0);
    assert_eq!(plan.preview.counts.unchanged, 2);
}

#[tokio::test]
async fn sun_panel_updates_once_when_the_projected_icon_changes() {
    let previous = ProjectedLink {
        sync_id: "mapping-1".to_string(),
        group_source_id: "single".to_string(),
        title: "Files".to_string(),
        url: "https://files.example.test/".to_string(),
        icon: Some("https://files.example.test/old.ico".to_string()),
    };
    let mut current = previous.clone();
    current.icon = Some("https://files.example.test/new.ico".to_string());
    let plan = sun_plan_with_unreadable_remote_icon(
        current,
        fingerprint(&previous),
        "https://files.example.test/",
    )
    .await;
    assert_eq!(plan.preview.counts.update, 1);
    assert_eq!(plan.preview.counts.unchanged, 1);
}

#[tokio::test]
async fn sun_panel_still_detects_readable_remote_drift_with_an_unreadable_icon() {
    let link = ProjectedLink {
        sync_id: "mapping-1".to_string(),
        group_source_id: "single".to_string(),
        title: "Files".to_string(),
        url: "https://files.example.test/".to_string(),
        icon: Some("https://files.example.test/__assets__/website_icon.uuid.ico".to_string()),
    };
    let plan = sun_plan_with_unreadable_remote_icon(
        link.clone(),
        fingerprint(&link),
        "https://changed.example.test/",
    )
    .await;
    assert_eq!(plan.preview.counts.update, 1);
    assert_eq!(plan.preview.counts.unchanged, 1);
}

#[test]
fn projection_reports_eligible_mappings_without_stable_sync_ids() {
    let config = json!({
        "host_mappings": [
            {"host":"missing.example.test","service_role":"app","disabled":false},
            {"host":"invalid.example.test","sync_id":"invalid","service_role":"app","disabled":false},
            {"host":"valid.example.test","sync_id":"11111111-1111-4111-8111-111111111111","service_role":"app","disabled":false},
            {"host":"off.example.test","service_role":"app","disabled":true},
            {"host":"auth.example.test","service_role":"auth","disabled":false}
        ]
    });
    assert_eq!(eligible_mappings_missing_sync_id(&config), 2);
}

#[test]
fn sun_panel_names_are_stable_and_connection_scoped() {
    assert_eq!(
        deterministic_name("a", "mapping"),
        deterministic_name("a", "mapping")
    );
    assert_ne!(
        deterministic_name("a", "mapping"),
        deterministic_name("b", "mapping")
    );
}

#[test]
fn ownership_plan_is_idempotent_and_recreates_missing_remote_objects() {
    let projected_group = ProjectedGroup {
        source_id: "namespace:apps".to_string(),
        name: "fn-knock / apps".to_string(),
    };
    let projected_link = ProjectedLink {
        sync_id: "11111111-1111-4111-8111-111111111111".to_string(),
        group_source_id: projected_group.source_id.clone(),
        title: "Files".to_string(),
        url: "https://files.example.test".to_string(),
        icon: None,
    };
    let projection = PanelLinkProjection {
        revision: "source-revision".to_string(),
        groups: vec![projected_group.clone()],
        links: vec![projected_link.clone()],
        warnings: Vec::new(),
    };
    let mut managed = ManagedState::default();
    managed.groups.insert(
        projected_group.source_id.clone(),
        ManagedObject {
            remote_id: "group-1".to_string(),
            fingerprint: fingerprint(&projected_group),
            title: projected_group.name.clone(),
            ..ManagedObject::default()
        },
    );
    managed.links.insert(
        projected_link.sync_id.clone(),
        ManagedObject {
            remote_id: "link-1".to_string(),
            remote_group_id: Some("group-1".to_string()),
            fingerprint: fingerprint(&projected_link),
            title: projected_link.title.clone(),
        },
    );
    let mut remote = RemoteSnapshot::default();
    remote.groups.insert(
        projected_group.source_id.clone(),
        RemoteObject {
            remote_id: "group-1".to_string(),
            fingerprint: fingerprint(&projected_group),
            exists: true,
        },
    );
    remote.links.insert(
        projected_link.sync_id.clone(),
        RemoteObject {
            remote_id: "link-1".to_string(),
            fingerprint: fingerprint(&projected_link),
            exists: true,
        },
    );
    let connection = connection(
        PanelProvider::OneNav,
        "http://localhost".to_string(),
        "/api",
    );
    let capabilities = AdapterRegistry::resolve(PanelProvider::OneNav).capabilities();
    let plan = build_plan(
        &connection,
        projection.clone(),
        managed.clone(),
        remote.clone(),
        &capabilities,
    );
    assert_eq!(plan.preview.counts.unchanged, 2);
    assert_eq!(plan.preview.counts.create, 0);
    assert_eq!(plan.preview.counts.update, 0);

    remote
        .links
        .get_mut(&projected_link.sync_id)
        .unwrap()
        .exists = false;
    let plan = build_plan(&connection, projection, managed, remote, &capabilities);
    assert!(plan.preview.actions.iter().any(|action| {
        action.kind == PlanActionKind::Create
            && action.source_id.as_deref() == Some(projected_link.sync_id.as_str())
    }));
}

#[test]
fn deterministic_remote_markers_recover_lost_local_ownership() {
    let projected_group = ProjectedGroup {
        source_id: "single".to_string(),
        name: "fn-knock".to_string(),
    };
    let projected_link = ProjectedLink {
        sync_id: "11111111-1111-4111-8111-111111111111".to_string(),
        group_source_id: projected_group.source_id.clone(),
        title: "Files".to_string(),
        url: "https://files.example.test".to_string(),
        icon: None,
    };
    let recovered_group = ManagedObject {
        remote_id: "10".to_string(),
        remote_group_id: None,
        fingerprint: fingerprint(&projected_group),
        title: projected_group.name.clone(),
    };
    let recovered_link = ManagedObject {
        remote_id: "20".to_string(),
        remote_group_id: Some("10".to_string()),
        fingerprint: fingerprint(&projected_link),
        title: projected_link.title.clone(),
    };
    let mut remote = RemoteSnapshot::default();
    remote
        .recovered
        .groups
        .insert(projected_group.source_id.clone(), recovered_group.clone());
    remote
        .recovered
        .links
        .insert(projected_link.sync_id.clone(), recovered_link.clone());
    remote.groups.insert(
        projected_group.source_id.clone(),
        RemoteObject {
            remote_id: recovered_group.remote_id.clone(),
            fingerprint: recovered_group.fingerprint.clone(),
            exists: true,
        },
    );
    remote.links.insert(
        projected_link.sync_id.clone(),
        RemoteObject {
            remote_id: recovered_link.remote_id.clone(),
            fingerprint: recovered_link.fingerprint.clone(),
            exists: true,
        },
    );
    let plan = build_plan(
        &connection(
            PanelProvider::OneNav,
            "https://nav.example.test".to_string(),
            "/api",
        ),
        PanelLinkProjection {
            revision: "revision".to_string(),
            groups: vec![projected_group],
            links: vec![projected_link],
            warnings: Vec::new(),
        },
        ManagedState::default(),
        remote,
        &AdapterRegistry::resolve(PanelProvider::OneNav).capabilities(),
    );

    assert_eq!(plan.preview.counts.unchanged, 2);
    assert_eq!(plan.preview.counts.create, 0);
    assert_eq!(plan.managed.groups["single"].remote_id, "10");
    assert_eq!(
        plan.managed.links["11111111-1111-4111-8111-111111111111"].remote_id,
        "20"
    );
}

#[test]
fn stale_objects_are_deleted_only_when_the_adapter_supports_it() {
    let mut managed = ManagedState::default();
    managed.links.insert(
        "removed-mapping".to_string(),
        ManagedObject {
            remote_id: "remote-link".to_string(),
            title: "Removed".to_string(),
            ..ManagedObject::default()
        },
    );
    let mut remote = RemoteSnapshot::default();
    remote.links.insert(
        "removed-mapping".to_string(),
        RemoteObject {
            remote_id: "remote-link".to_string(),
            fingerprint: String::new(),
            exists: true,
        },
    );
    let projection = PanelLinkProjection {
        revision: "revision".to_string(),
        groups: Vec::new(),
        links: Vec::new(),
        warnings: Vec::new(),
    };
    let one_nav = build_plan(
        &connection(
            PanelProvider::OneNav,
            "http://localhost".to_string(),
            "/api",
        ),
        projection.clone(),
        managed.clone(),
        remote.clone(),
        &AdapterRegistry::resolve(PanelProvider::OneNav).capabilities(),
    );
    assert_eq!(one_nav.preview.counts.delete, 1);
    assert_eq!(one_nav.preview.counts.residual, 0);

    let sun = build_plan(
        &connection(
            PanelProvider::SunPanel,
            "http://localhost".to_string(),
            "/openapi/v1",
        ),
        projection,
        managed,
        remote,
        &AdapterRegistry::resolve(PanelProvider::SunPanel).capabilities(),
    );
    assert_eq!(sun.preview.counts.delete, 0);
    assert_eq!(sun.preview.counts.residual, 1);
    assert!(!sun.preview.warnings.is_empty());
}

#[test]
fn sun_panel_reports_group_renames_as_residuals() {
    let group = ProjectedGroup {
        source_id: "apps".to_string(),
        name: "fn-knock · Apps renamed".to_string(),
    };
    let mut managed = ManagedState::default();
    managed.groups.insert(
        group.source_id.clone(),
        ManagedObject {
            remote_id: "45".to_string(),
            fingerprint: "old-fingerprint".to_string(),
            title: "fn-knock · Apps".to_string(),
            ..ManagedObject::default()
        },
    );
    let mut remote = RemoteSnapshot::default();
    remote.groups.insert(
        group.source_id.clone(),
        RemoteObject {
            remote_id: "45".to_string(),
            fingerprint: "old-fingerprint".to_string(),
            exists: true,
        },
    );
    let plan = build_plan(
        &connection(
            PanelProvider::SunPanel,
            "http://localhost".to_string(),
            "/openapi/v1",
        ),
        PanelLinkProjection {
            revision: "revision".to_string(),
            groups: vec![group],
            links: Vec::new(),
            warnings: Vec::new(),
        },
        managed,
        remote,
        &AdapterRegistry::resolve(PanelProvider::SunPanel).capabilities(),
    );
    assert_eq!(plan.preview.counts.update, 0);
    assert_eq!(plan.preview.counts.residual, 1);
}

#[test]
fn unowned_name_conflicts_block_the_plan() {
    let projection = PanelLinkProjection {
        revision: "revision".to_string(),
        groups: Vec::new(),
        links: Vec::new(),
        warnings: Vec::new(),
    };
    let remote = RemoteSnapshot {
        conflicts: vec![PlanAction {
            kind: PlanActionKind::Conflict,
            object_type: "link".to_string(),
            source_id: Some("mapping".to_string()),
            remote_id: Some("unowned".to_string()),
            title: "Existing".to_string(),
            detail: "not owned".to_string(),
        }],
        ..RemoteSnapshot::default()
    };
    let plan = build_plan(
        &connection(
            PanelProvider::VanNav,
            "http://localhost".to_string(),
            "/api",
        ),
        projection,
        ManagedState::default(),
        remote,
        &AdapterRegistry::resolve(PanelProvider::VanNav).capabilities(),
    );
    assert_eq!(plan.preview.counts.conflict, 1);
    assert!(!plan.preview.can_apply);
}

#[tokio::test]
async fn provider_probes_use_documented_paths_and_authentication() {
    let (base_url, request) =
        mock_json_response(r#"{"success":true,"data":{"version":"1.7"}}"#).await;
    let context = AdapterContext {
        connection: connection(PanelProvider::SunPanel, base_url, "/openapi/v1"),
        credential: "sun-secret".to_string(),
    };
    AdapterRegistry::resolve(PanelProvider::SunPanel)
        .probe(&context)
        .await
        .unwrap();
    let request = request.await.unwrap();
    assert!(request.starts_with("POST /openapi/v1/version "));
    assert!(request.to_ascii_lowercase().contains("token: sun-secret"));

    let (base_url, request) = mock_json_response(r#"{"code":0,"data":[]}"#).await;
    let context = AdapterContext {
        connection: connection(PanelProvider::OneNav, base_url, "/api"),
        credential: "one-secret".to_string(),
    };
    AdapterRegistry::resolve(PanelProvider::OneNav)
        .probe(&context)
        .await
        .unwrap();
    let request = request.await.unwrap();
    assert!(request.starts_with("POST /api/category_list "));
    assert!(request.contains("token=one-secret"));

    let (base_url, request) =
        mock_json_response(r#"{"success":true,"data":{"categories":[]}}"#).await;
    let context = AdapterContext {
        connection: connection(PanelProvider::VanNav, base_url, "/api"),
        credential: "van-secret".to_string(),
    };
    AdapterRegistry::resolve(PanelProvider::VanNav)
        .probe(&context)
        .await
        .unwrap();
    let request = request.await.unwrap();
    assert!(request.starts_with("GET /api/admin/all "));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer van-secret")
    );
}

#[tokio::test]
async fn sun_panel_apply_uses_official_item_group_and_item_contract() {
    let (base_url, requests) = mock_json_responses(vec![
        json!({"code": 0}).to_string(),
        json!({"code": 0, "data": {"itemGroupID": 45, "title": "NAS"}}).to_string(),
        json!({"code": 0}).to_string(),
    ])
    .await;
    let context = AdapterContext {
        connection: connection(PanelProvider::SunPanel, base_url, "/openapi/v1"),
        credential: "sun-secret".to_string(),
    };
    let projection = PanelLinkProjection {
        revision: "revision".to_string(),
        groups: vec![ProjectedGroup {
            source_id: "single".to_string(),
            name: "NAS".to_string(),
        }],
        links: vec![ProjectedLink {
            sync_id: "mapping-1".to_string(),
            group_source_id: "single".to_string(),
            title: "Files".to_string(),
            url: "https://files.example.test".to_string(),
            icon: Some("https://files.example.test/favicon.ico".to_string()),
        }],
        warnings: Vec::new(),
    };
    let plan = build_plan(
        &context.connection,
        projection,
        ManagedState::default(),
        RemoteSnapshot::default(),
        &AdapterRegistry::resolve(PanelProvider::SunPanel).capabilities(),
    );
    let checkpoint = ApplyCheckpoint::new(ManagedState::default());
    let managed = AdapterRegistry::resolve(PanelProvider::SunPanel)
        .apply(&context, &plan, &checkpoint)
        .await
        .unwrap();
    assert_eq!(managed.groups["single"].remote_id, "45");
    assert!(managed.links.contains_key("mapping-1"));

    let requests = requests.await.unwrap();
    assert!(requests[0].starts_with("POST /openapi/v1/itemGroup/create "));
    assert!(requests[0].contains("\"title\":\"NAS\""));
    assert!(requests[0].contains("\"onlyName\":"));
    assert!(requests[1].starts_with("POST /openapi/v1/itemGroup/getInfo "));
    assert!(requests[2].starts_with("POST /openapi/v1/item/create "));
    assert!(requests[2].contains("\"itemGroupID\":45"));
    assert!(requests[2].contains("\"iconUrl\":\"https://files.example.test/favicon.ico\""));
}

#[tokio::test]
async fn van_nav_apply_uses_catelog_contract_and_preserves_partial_ownership() {
    let (base_url, requests) = mock_json_responses(vec![
        json!({"success": true, "data": {"id": 3}}).to_string(),
        json!({"success": false, "message": "rejected"}).to_string(),
    ])
    .await;
    let context = AdapterContext {
        connection: connection(PanelProvider::VanNav, base_url, "/api"),
        credential: "van-secret".to_string(),
    };
    let projection = PanelLinkProjection {
        revision: "revision".to_string(),
        groups: vec![ProjectedGroup {
            source_id: "single".to_string(),
            name: "NAS".to_string(),
        }],
        links: vec![ProjectedLink {
            sync_id: "mapping-1".to_string(),
            group_source_id: "single".to_string(),
            title: "Files".to_string(),
            url: "https://files.example.test".to_string(),
            icon: Some("https://files.example.test/favicon.ico".to_string()),
        }],
        warnings: Vec::new(),
    };
    let plan = build_plan(
        &context.connection,
        projection,
        ManagedState::default(),
        RemoteSnapshot::default(),
        &AdapterRegistry::resolve(PanelProvider::VanNav).capabilities(),
    );
    let checkpoint = ApplyCheckpoint::new(ManagedState::default());
    assert!(
        AdapterRegistry::resolve(PanelProvider::VanNav)
            .apply(&context, &plan, &checkpoint)
            .await
            .is_err()
    );
    let partial = checkpoint.latest();
    assert_eq!(partial.groups["single"].remote_id, "3");
    assert!(partial.links.is_empty());

    let requests = requests.await.unwrap();
    assert!(requests[0].starts_with("POST /api/admin/catelog "));
    assert!(requests[1].starts_with("POST /api/admin/tool "));
    assert!(requests[1].contains("\"catelogId\":3"));
    assert!(requests[1].contains("\"logo\":\"https://files.example.test/favicon.ico\""));
}

#[tokio::test]
async fn one_nav_inspection_follows_bounded_pagination() {
    let categories = (0..200)
        .map(|index| json!({"id": index + 1, "name": format!("category-{index}")}))
        .collect::<Vec<_>>();
    let (base_url, requests) = mock_json_responses(vec![
        json!({"code": 0, "data": categories}).to_string(),
        json!({"code": 0, "data": []}).to_string(),
        json!({"code": 0, "data": []}).to_string(),
    ])
    .await;
    let context = AdapterContext {
        connection: connection(PanelProvider::OneNav, base_url, "/api"),
        credential: "one-secret".to_string(),
    };
    AdapterRegistry::resolve(PanelProvider::OneNav)
        .inspect(
            &context,
            &ManagedState::default(),
            &PanelLinkProjection {
                revision: "revision".to_string(),
                groups: Vec::new(),
                links: Vec::new(),
                warnings: Vec::new(),
            },
        )
        .await
        .unwrap();
    let requests = requests.await.unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].contains("page=1&limit=200"));
    assert!(requests[1].contains("page=2&limit=200"));
    assert!(requests[2].starts_with("POST /api/link_list "));
}

#[test]
fn panel_credentials_are_encrypted_and_private() {
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::new(directory.path());
    store.write("connection-id", "plain-secret-token").unwrap();
    assert_eq!(
        store.read("connection-id").unwrap().as_deref(),
        Some("plain-secret-token")
    );
    let encrypted = std::fs::read(
        directory
            .path()
            .join("panel-sync/credentials/connection-id.enc"),
    )
    .unwrap();
    assert!(!String::from_utf8_lossy(&encrypted).contains("plain-secret-token"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(
            directory
                .path()
                .join("panel-sync/credentials/connection-id.enc"),
        )
        .unwrap()
        .permissions()
        .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
