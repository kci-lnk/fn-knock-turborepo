use super::*;

#[tokio::test]
async fn ip_location_lock_can_only_be_released_by_its_owner() {
    let (_directory, store) = open_test_store().await;
    let ip = "203.0.113.10";

    assert!(
        store
            .acquire_ip_location_lock(ip, "owner-a", 60)
            .await
            .expect("acquire initial lock")
    );
    store
        .release_ip_location_lock(ip, "owner-b")
        .await
        .expect("ignore non-owner release");
    assert!(
        !store
            .acquire_ip_location_lock(ip, "owner-c", 60)
            .await
            .expect("lock remains owned")
    );

    store
        .release_ip_location_lock(ip, "owner-a")
        .await
        .expect("release owned lock");
    assert!(
        store
            .acquire_ip_location_lock(ip, "owner-c", 60)
            .await
            .expect("acquire after owner release")
    );
}
