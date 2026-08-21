use super::*;

#[allow(dead_code)]
pub fn new_login_session(
    totp_id: &str,
    credential_name: &str,
    ip: &str,
    user_agent: &str,
    ttl_seconds: i64,
) -> LoginSession {
    LoginSession {
        totp_id: totp_id.to_string(),
        method: "TOTP".to_string(),
        credential_id: totp_id.to_string(),
        credential_name: credential_name.to_string(),
        linked_totp_name: None,
        access_scopes: None,
        subdomain_access: None,
        grant_type: Some("browser_session".to_string()),
        post_login_ip_grant_mode: None,
        post_login_ip_grant_record_id: None,
        stream_access_expires_at: None,
        comment: None,
        ip: ip.to_string(),
        user_agent: user_agent.to_string(),
        login_time: now_iso(),
        expires_at: Some(iso_after_seconds(ttl_seconds)),
        ip_location: None,
    }
}
