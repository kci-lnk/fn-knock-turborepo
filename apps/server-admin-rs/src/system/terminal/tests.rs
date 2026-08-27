use std::fs;

use super::{
    domain::{
        AttachmentRole, AuthMethod, SessionPhase, TerminalErrorCode, TerminalEvent,
        TerminalEventType,
    },
    secrets::{CredentialKind, TerminalSecretStore},
};

#[test]
fn serializes_stable_wire_enums() {
    assert_eq!(
        serde_json::to_string(&AuthMethod::PrivateKey).unwrap(),
        r#""privateKey""#
    );
    assert_eq!(
        serde_json::to_string(&SessionPhase::VerifyingHostKey).unwrap(),
        r#""verifyingHostKey""#
    );
    assert_eq!(
        serde_json::to_string(&TerminalErrorCode::HostKeyRequired).unwrap(),
        r#""host_key_required""#
    );
    assert_eq!(
        serde_json::to_string(&AttachmentRole::Controller).unwrap(),
        r#""controller""#
    );
}

#[test]
fn status_event_carries_terminal_failure_details() {
    let event = TerminalEvent {
        kind: TerminalEventType::Status,
        cursor: 7,
        data_base64: None,
        reset: false,
        phase: Some(SessionPhase::Lost),
        error_code: Some(TerminalErrorCode::SessionLost),
        error_message: Some("connection lost".to_string()),
        exit_code: None,
        role: None,
        generation: None,
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["type"], "status");
    assert_eq!(value["phase"], "lost");
    assert_eq!(value["errorCode"], "session_lost");
    assert_eq!(value["errorMessage"], "connection lost");
}

#[test]
fn encrypts_and_domain_separates_terminal_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let store = TerminalSecretStore::new(directory.path().join("terminal"));
    store
        .write("target-a", CredentialKind::Password, b"secret")
        .unwrap();
    store
        .write("target-a", CredentialKind::PrivateKey, b"private-key")
        .unwrap();
    assert_eq!(
        store
            .read("target-a", CredentialKind::Password)
            .unwrap()
            .unwrap(),
        b"secret"
    );
    assert!(
        store
            .read("target-b", CredentialKind::Password)
            .unwrap()
            .is_none()
    );
    let raw = fs::read_to_string(
        directory
            .path()
            .join("terminal/secrets/target-target-a.enc"),
    )
    .unwrap();
    assert!(!raw.contains("secret"));
    assert!(!raw.contains("private-key"));

    fs::copy(
        directory
            .path()
            .join("terminal/secrets/target-target-a.enc"),
        directory
            .path()
            .join("terminal/secrets/target-target-b.enc"),
    )
    .unwrap();
    assert!(store.read("target-b", CredentialKind::PrivateKey).is_err());
}

#[test]
fn all_target_credentials_are_committed_as_one_encrypted_bundle() {
    let directory = tempfile::tempdir().unwrap();
    let store = TerminalSecretStore::new(directory.path().join("terminal"));
    store
        .write("target-a", CredentialKind::PrivateKey, b"private-key")
        .unwrap();
    store
        .write("target-a", CredentialKind::Passphrase, b"passphrase")
        .unwrap();
    let files = fs::read_dir(directory.path().join("terminal/secrets"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_name(), "target-target-a.enc");
    assert_eq!(
        store
            .read("target-a", CredentialKind::PrivateKey)
            .unwrap()
            .as_deref(),
        Some(b"private-key".as_slice())
    );
    assert_eq!(
        store
            .read("target-a", CredentialKind::Passphrase)
            .unwrap()
            .as_deref(),
        Some(b"passphrase".as_slice())
    );
}

#[test]
fn credential_store_rejects_path_traversal() {
    let directory = tempfile::tempdir().unwrap();
    let store = TerminalSecretStore::new(directory.path().join("terminal"));
    for id in ["", "../escape", "target/escape", "target\\escape"] {
        assert!(store.write(id, CredentialKind::Password, b"x").is_err());
        assert!(store.read(id, CredentialKind::Password).is_err());
        assert!(store.delete(id, CredentialKind::Password).is_err());
    }
    assert!(!directory.path().join("escape.enc").exists());
}
