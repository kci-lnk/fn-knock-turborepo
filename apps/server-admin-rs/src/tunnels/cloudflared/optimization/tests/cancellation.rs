#[test]
fn cancelled_scan_requests_do_not_block_the_next_serialized_scan() {
    assert!(scan_job_active(
        &json!({ "status": "running", "cancelRequested": false })
    ));
    assert!(!scan_job_active(
        &json!({ "status": "running", "cancelRequested": true })
    ));
    assert!(!scan_job_active(&json!({ "status": "completed" })));
}
use super::*;
