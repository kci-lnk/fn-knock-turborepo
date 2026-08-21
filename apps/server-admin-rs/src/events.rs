pub(crate) mod routes;

pub(crate) use routes::*;

/// Payload fields that carry source IPs for each system event type.
///
/// Keep this mapping in the event domain so persistence hydration, background
/// IP-location synchronization, and notification rendering all agree on the
/// same fields.
pub(crate) fn system_event_ip_fields(
    event_type: Option<&str>,
) -> &'static [(&'static str, &'static str)] {
    match event_type.unwrap_or("") {
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => {
            &[("from_ip", "from_ip_location"), ("to_ip", "to_ip_location")]
        }
        "FN_EVENT_AUTH_LOGIN_SUCCESS"
        | "FN_EVENT_AUTH_LOGOUT"
        | "FN_EVENT_AUTH_LOGIN_FAILURE"
        | "FN_EVENT_SECURITY_SCANNER_BLOCKED"
        | "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
        | "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"
        | "FN_EVENT_WAF_BLOCKED"
        | "FN_EVENT_SSH_LOGIN_SUCCESS"
        | "FN_EVENT_SSH_LOGIN_FAILURE"
        | "FN_EVENT_SSH_IP_BLOCKED" => &[("ip", "ip_location")],
        _ => &[],
    }
}
