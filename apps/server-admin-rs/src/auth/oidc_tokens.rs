use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

pub(crate) fn create_oidc_id(prefix: &str) -> String {
    format!("{prefix}_{}", hex::encode(rand::random::<[u8; 10]>()))
}

pub(crate) fn create_public_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_expected_oidc_id_and_token_shapes() {
        let id = create_oidc_id("state");
        let suffix = id.strip_prefix("state_").unwrap();
        assert_eq!(suffix.len(), 20);
        assert!(suffix.chars().all(|ch| ch.is_ascii_hexdigit()));

        let token = create_public_token();
        assert_eq!(token.len(), 43);
        assert!(
            token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        );
    }
}
