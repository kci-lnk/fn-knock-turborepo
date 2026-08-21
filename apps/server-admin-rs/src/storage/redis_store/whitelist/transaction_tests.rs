use super::*;

fn region_record() -> WhitelistRegionGroupRecord {
    WhitelistRegionGroupRecord {
        id: "whitelist-region:cas".to_string(),
        regions: vec![WhitelistRegionInput {
            province: "广东".to_string(),
            query_city: None,
            operator: None,
        }],
        cidrs: vec!["192.0.2.0/24".to_string()],
        policy_id: String::new(),
        policy: None,
        source_cidr_count: 1,
        range_count: 1,
        expire_at: None,
        source: "manual".to_string(),
        created_at: 1,
        updated_at: 1,
        status: "active".to_string(),
        comment: None,
    }
}

#[tokio::test]
async fn stale_region_cas_cannot_overwrite_compatibility_or_typed_state() {
    let directory = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(directory.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let original = region_record();
    store
        .insert_whitelist_region_group(&original)
        .await
        .expect("insert original region");

    let mut fresh = original.clone();
    fresh.updated_at = 2;
    fresh.comment = Some("fresh".to_string());
    store
        .insert_whitelist_region_group(&fresh)
        .await
        .expect("write concurrent replacement");

    let mut stale_tombstone = original.clone();
    stale_tombstone.status = "deleted".to_string();
    stale_tombstone.cidrs.clear();
    stale_tombstone.source_cidr_count = 0;
    stale_tombstone.range_count = 0;
    let mut pipeline = redis::pipe();
    pipeline
        .hset(
            WHITELIST_REGION_GROUP_RECORDS,
            &original.id,
            serde_json::to_string(&stale_tombstone).unwrap(),
        )
        .ignore();
    pipeline
        .zrem(WHITELIST_REGION_GROUP_ORDER, &original.id)
        .ignore();

    let matched = store
        .execute_whitelist_region_pipeline_if_current(
            &original,
            TypedWhitelistMutation::Upsert(typed_whitelist_region(&stale_tombstone).unwrap()),
            pipeline,
        )
        .await
        .expect("run stale region CAS");
    assert!(!matched);

    let current = store
        .get_whitelist_region_group(&original.id)
        .await
        .unwrap()
        .expect("current region");
    assert_eq!(current.status, "active");
    assert_eq!(current.comment.as_deref(), Some("fresh"));
    let typed = store
        .typed
        .typed_whitelist
        .load_one("region", &original.id)
        .await
        .unwrap()
        .expect("typed current region");
    assert_eq!(typed.document_json, serde_json::to_string(&fresh).unwrap());
}
