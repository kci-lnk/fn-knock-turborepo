use serde_json::{Map, Value, json};

use super::{
    client::{
        LdapAuthError, authenticate, binary_subject_for_test, client_error_is_unavailable_for_test,
        direct_principal_for_test, split_pem_certificates_for_test,
    },
    provider::{
        build_new_provider, build_updated_provider, mask_provider, normalize_server_for_test,
        provider_config, provider_ready,
    },
    runtime::binding_matches_identity_for_test,
};

const TEST_CA_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIICrDCCAZQCCQCDA9ZZbkN1MzANBgkqhkiG9w0BAQsFADAYMRYwFAYDVQQDDA1G\n\
bktub2NrVGVzdENBMB4XDTI2MDgwMjE3NTUzOFoXDTI2MDgwMzE3NTUzOFowGDEW\n\
MBQGA1UEAwwNRm5Lbm9ja1Rlc3RDQTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCC\n\
AQoCggEBALVVB920RzJTleQ2/xuLfPbdV0DWw5RR1UE0bAKIHONdTbXATAQe6m+C\n\
23gf75xeoujv+WScTmOBS8qxm00ha/CEugXguhT3Xqzjyp0hqHT9oEKtJ/a6NIxr\n\
nmtvBGQWoUtD4S75/1GFDZxahkDTLZ8N7jQGiFmVCL0osLCCZYbsEI8iF12iJE0U\n\
g54FBJMNjmlb+w2PJgZj7VfebkyuwrsN+yTQZY136piyd/VceP21jGMoU1U2TQIw\n\
n7mx0MllXRw/R9blTRsM0rL1l7CgzXP55BB4TqfPiHlfEnttrX8VeA2e3pHmKoEg\n\
kwyV7g29zI55o7/anujOMqtEw0xnQuUCAwEAATANBgkqhkiG9w0BAQsFAAOCAQEA\n\
ryIaXUI90V4oAab9jzKR4nwmJW4GXtKMTwSF6e3CXCQWebuNqrNgBryvbWXybY8x\n\
i25EAz/I44i0Nn+T4gVL+ChCD7UPeWIM+CvYTZ0BGkBGZPoGmC39913a0RBc/zv6\n\
eX3RnoQpRynpelvRhCqhrZte4NoHtmUx9+2k7INiF1XgB+jJVOWgFAoTJ4uA3YoZ\n\
4HENziZezL7MAXfh0DMZWXdXd7qF5fU2oZwBWyeMrLerrUdTrexUvlPzXb5QUw1X\n\
Pj5tbEZIP/7Y9Rtjvv959laZJ7KsBb+QVQ6Lo2rbZwrbyVwTDfxhgA7reDNmWnv2\n\
qFkogrl9uqcXxRS2G9aGdA==\n\
-----END CERTIFICATE-----";

fn provider_input() -> Map<String, Value> {
    json!({
        "type": "openldap",
        "name": "Company LDAP",
        "enabled": true,
        "connection_config": {
            "servers": ["ldaps://ldap.example.com:636"],
            "transport": "ldaps",
            "bind_mode": "search",
            "base_dn": "dc=example,dc=com",
            "user_filter": "(&(objectClass=person)(uid={username}))",
            "service_bind_dn": "cn=reader,dc=example,dc=com",
            "service_bind_password": "super-secret",
            "subject_attribute": "entryUUID",
            "username_attribute": "uid",
            "display_name_attribute": "cn",
            "email_attribute": "mail",
            "ca_pem": TEST_CA_PEM,
        }
    })
    .as_object()
    .cloned()
    .unwrap()
}

#[test]
fn rejects_plaintext_and_transport_mismatch() {
    assert!(normalize_server_for_test("ldap.example.com:636", "ldaps").is_ok());
    assert!(normalize_server_for_test("ldap://ldap.example.com:389", "ldaps").is_err());
    assert!(normalize_server_for_test("ldaps://ldap.example.com:636", "starttls").is_err());
    assert!(normalize_server_for_test("ldap://user:secret@ldap.example.com", "starttls").is_err());
}

#[test]
fn rejects_insecure_or_incomplete_stored_provider_config() {
    let mut provider = build_new_provider(&provider_input()).expect("valid provider");
    provider["connection_config"]["transport"] = json!("plaintext");
    provider["connection_config"]["servers"] = json!(["ldap://ldap.example.com:389"]);
    assert!(provider_config(&provider).is_err());
    assert!(!provider_ready(&provider));

    let mut provider = build_new_provider(&provider_input()).expect("valid provider");
    provider["connection_config"]["subject_attribute"] = json!("");
    assert!(provider_config(&provider).is_err());
}

#[test]
fn validates_and_masks_provider_secrets() {
    let provider = build_new_provider(&provider_input()).expect("valid provider");
    assert!(provider_ready(&provider));
    let masked = mask_provider(provider.clone());
    assert_eq!(
        masked.pointer("/connection_config/service_bind_password"),
        Some(&json!("********"))
    );
    assert_eq!(
        masked.pointer("/connection_config/ca_pem"),
        Some(&json!(TEST_CA_PEM))
    );

    let patch = json!({
        "connection_config": {
            "service_bind_password": "",
            "ca_pem": "",
        }
    });
    let updated = build_updated_provider(provider, patch.as_object().unwrap())
        .expect("masked values preserve secrets");
    assert_eq!(
        updated.pointer("/connection_config/service_bind_password"),
        Some(&json!("super-secret"))
    );
    assert_eq!(
        updated.pointer("/connection_config/ca_pem"),
        Some(&json!(""))
    );
}

#[test]
fn requires_safe_templates_and_complete_enabled_configuration() {
    let mut input = provider_input();
    input
        .get_mut("connection_config")
        .and_then(Value::as_object_mut)
        .unwrap()
        .insert("user_filter".into(), json!("(uid=literal)"));
    assert!(build_new_provider(&input).is_err());

    let mut direct = provider_input();
    let config = direct
        .get_mut("connection_config")
        .and_then(Value::as_object_mut)
        .unwrap();
    config.insert("bind_mode".into(), json!("direct"));
    config.insert("direct_bind_template".into(), json!("literal@example.com"));
    assert!(build_new_provider(&direct).is_err());

    let mut invalid_ca = provider_input();
    invalid_ca
        .get_mut("connection_config")
        .and_then(Value::as_object_mut)
        .unwrap()
        .insert("ca_pem".into(), json!("not a certificate"));
    assert!(build_new_provider(&invalid_ca).is_err());
}

#[test]
fn escapes_filter_input_and_encodes_binary_ad_guid() {
    assert_eq!(
        ldap3::ldap_escape("alice*)(uid=*)").as_ref(),
        "alice\\2a\\29\\28uid=\\2a\\29"
    );
    assert_eq!(binary_subject_for_test(&[0, 1, 2, 250, 255]), "AAEC-v8");
    assert_eq!(
        direct_principal_for_test("{username}@example.com", "alice@example.net"),
        "alice@example.net@example.com"
    );
    assert_eq!(
        direct_principal_for_test("uid={username},dc=example,dc=com", " leading"),
        "uid=\\20leading,dc=example,dc=com"
    );
}

#[test]
fn splits_private_ca_bundle() {
    let bundle = "-----BEGIN CERTIFICATE-----\nAAA=\n-----END CERTIFICATE-----\n\n-----BEGIN CERTIFICATE-----\nBBB=\n-----END CERTIFICATE-----";
    assert_eq!(split_pem_certificates_for_test(bundle).len(), 2);
    assert!(split_pem_certificates_for_test("not a certificate").is_empty());
}

#[test]
fn fails_over_only_for_connection_class_client_errors() {
    assert!(client_error_is_unavailable_for_test(
        std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset").into(),
    ));
    assert!(!client_error_is_unavailable_for_test(
        ldap3::LdapError::FilterParsing,
    ));
}

#[test]
fn rejects_mismatched_binding_subject_or_provider() {
    let binding = json!({
        "provider_id": "ldap_provider_a",
        "subject_key": "subject-a",
    });
    assert!(binding_matches_identity_for_test(
        &binding,
        "ldap_provider_a",
        "subject-a"
    ));
    assert!(!binding_matches_identity_for_test(
        &binding,
        "ldap_provider_b",
        "subject-a"
    ));
    assert!(!binding_matches_identity_for_test(
        &binding,
        "ldap_provider_a",
        "subject-b"
    ));
}

#[tokio::test]
#[ignore = "requires the temporary OpenLDAP TLS fixture described by FN_KNOCK_LDAP_TEST_* variables"]
async fn openldap_tls_search_direct_unique_result_and_failover() {
    let ca_path = std::env::var("FN_KNOCK_LDAP_TEST_CA_PATH")
        .expect("FN_KNOCK_LDAP_TEST_CA_PATH must point to the fixture CA certificate");
    let ca_pem =
        std::fs::read_to_string(ca_path).expect("fixture CA certificate should be readable");
    let ldaps_url = std::env::var("FN_KNOCK_LDAP_TEST_LDAPS_URL")
        .unwrap_or_else(|_| "ldaps://localhost:1636".into());
    let starttls_url = std::env::var("FN_KNOCK_LDAP_TEST_STARTTLS_URL")
        .unwrap_or_else(|_| "ldap://localhost:1389".into());
    let password =
        std::env::var("FN_KNOCK_LDAP_TEST_PASSWORD").unwrap_or_else(|_| "alice-secret".into());
    let base_dn = "dc=example,dc=org";

    let search_provider = json!({
        "id": "ldap_provider_openldap_test",
        "connection_config": {
            "servers": ["ldaps://127.0.0.1:9", ldaps_url],
            "transport": "ldaps",
            "bind_mode": "search",
            "base_dn": base_dn,
            "user_filter": "(&(objectClass=inetOrgPerson)(uid={username}))",
            "service_bind_dn": format!("cn=admin,{base_dn}"),
            "service_bind_password": "admin-secret",
            "direct_bind_template": "",
            "subject_attribute": "entryUUID",
            "username_attribute": "uid",
            "display_name_attribute": "cn",
            "email_attribute": "mail",
            "ca_pem": ca_pem,
        }
    });
    let search_profile = authenticate(&search_provider, "alice", &password)
        .await
        .expect("LDAPS search bind should fail over and authenticate");
    assert_eq!(search_profile.username, "alice");
    assert_eq!(search_profile.email.as_deref(), Some("alice@example.org"));
    assert!(!search_profile.subject.is_empty());

    let invalid_password = authenticate(&search_provider, "alice", "incorrect")
        .await
        .expect_err("an invalid directory password must be rejected");
    assert!(matches!(
        invalid_password,
        LdapAuthError::InvalidCredentials
    ));

    let mut ambiguous_provider = search_provider.clone();
    ambiguous_provider["connection_config"]["servers"] = json!([ldaps_url]);
    ambiguous_provider["connection_config"]["user_filter"] = json!("(|(uid={username})(uid=bob))");
    let ambiguous = authenticate(&ambiguous_provider, "alice", &password)
        .await
        .expect_err("a non-unique search result must be rejected");
    assert!(matches!(ambiguous, LdapAuthError::UserNotFound));

    let direct_provider = json!({
        "id": "ldap_provider_openldap_direct_test",
        "connection_config": {
            "servers": [starttls_url],
            "transport": "starttls",
            "bind_mode": "direct",
            "base_dn": base_dn,
            "user_filter": "(&(objectClass=inetOrgPerson)(uid={username}))",
            "service_bind_dn": "",
            "service_bind_password": "",
            "direct_bind_template": format!("uid={{username}},ou=people,{base_dn}"),
            "subject_attribute": "entryUUID",
            "username_attribute": "uid",
            "display_name_attribute": "cn",
            "email_attribute": "mail",
            "ca_pem": search_provider["connection_config"]["ca_pem"].clone(),
        }
    });
    let direct_profile = authenticate(&direct_provider, "alice", &password)
        .await
        .expect("StartTLS direct bind should authenticate and read the profile");
    assert_eq!(direct_profile.subject, search_profile.subject);
    assert_ne!(direct_profile.subject_key, search_profile.subject_key);
}
