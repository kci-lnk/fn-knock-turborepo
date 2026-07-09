use scrypt::{Params as ScryptParams, scrypt};
use subtle::ConstantTimeEq;

use crate::{crypto_utils::random_bytes, store::AuthPasswordCredential, time_utils};

const SCRYPT_N: u32 = 16_384;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;
const SCRYPT_KEY_LENGTH: usize = 64;
const SCRYPT_SALT_HEX_LENGTH: usize = 32;
const SCRYPT_HASH_HEX_LENGTH: usize = SCRYPT_KEY_LENGTH * 2;
const DUMMY_PASSWORD_SALT_HEX: &str = "000102030405060708090a0b0c0d0e0f";

pub(crate) fn make_auth_password_credential(
    account_id: &str,
    password: &str,
    created_at: Option<String>,
) -> anyhow::Result<AuthPasswordCredential> {
    let now = time_utils::now_iso();
    let salt = hex::encode(random_bytes::<16>());
    let hash = derive_password_hash(
        password,
        &salt,
        SCRYPT_N,
        SCRYPT_R,
        SCRYPT_P,
        SCRYPT_KEY_LENGTH,
    )?;
    Ok(AuthPasswordCredential {
        account_id: account_id.to_string(),
        algorithm: "scrypt".to_string(),
        salt,
        hash,
        n: SCRYPT_N,
        r: SCRYPT_R,
        p: SCRYPT_P,
        key_length: SCRYPT_KEY_LENGTH,
        created_at: created_at.unwrap_or_else(|| now.clone()),
        updated_at: now,
    })
}

pub(crate) fn verify_auth_password(
    password: &str,
    record: &AuthPasswordCredential,
) -> anyhow::Result<bool> {
    if record.algorithm != "scrypt" {
        return Ok(false);
    }
    let expected = derive_password_hash(
        password,
        &record.salt,
        record.n.max(2),
        record.r.max(1),
        record.p.max(1),
        record.key_length.max(1),
    )?;
    Ok(expected
        .as_bytes()
        .ct_eq(record.hash.as_bytes())
        .unwrap_u8()
        == 1)
}

pub(crate) fn is_supported_auth_password_credential(record: &AuthPasswordCredential) -> bool {
    !record.account_id.trim().is_empty()
        && record.algorithm == "scrypt"
        && record.n == SCRYPT_N
        && record.r == SCRYPT_R
        && record.p == SCRYPT_P
        && record.key_length == SCRYPT_KEY_LENGTH
        && record.salt.len() == SCRYPT_SALT_HEX_LENGTH
        && record.hash.len() == SCRYPT_HASH_HEX_LENGTH
        && is_hex_string(&record.salt)
        && is_hex_string(&record.hash)
}

pub(crate) fn consume_dummy_auth_password_hash(password: &str) -> anyhow::Result<()> {
    let _ = derive_password_hash(
        password,
        DUMMY_PASSWORD_SALT_HEX,
        SCRYPT_N,
        SCRYPT_R,
        SCRYPT_P,
        SCRYPT_KEY_LENGTH,
    )?;
    Ok(())
}

pub(crate) fn validate_auth_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 6 {
        return Err("passwordTooShort");
    }
    if password.len() > 128 {
        return Err("passwordTooLong");
    }
    if password.chars().any(char::is_whitespace) {
        return Err("passwordWhitespace");
    }
    if !password.chars().any(|value| value.is_ascii_alphabetic())
        || !password.chars().any(|value| value.is_ascii_digit())
    {
        return Err("passwordNeedsLettersAndNumbers");
    }
    Ok(())
}

fn derive_password_hash(
    password: &str,
    salt_hex: &str,
    n: u32,
    r: u32,
    p: u32,
    key_length: usize,
) -> anyhow::Result<String> {
    let salt = hex::decode(salt_hex)?;
    let log_n = n.ilog2() as u8;
    let params = ScryptParams::new(log_n, r, p)?;
    let mut output = vec![0u8; key_length];
    scrypt(password.as_bytes(), &salt, &params, &mut output)?;
    Ok(hex::encode(output))
}

fn is_hex_string(value: &str) -> bool {
    value.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_auth_password_rules() {
        assert!(validate_auth_password("abc123").is_ok());
        assert!(validate_auth_password("abc12").is_err());
        assert!(validate_auth_password("abcdef").is_err());
        assert!(validate_auth_password("123456").is_err());
        assert!(validate_auth_password("abc 123").is_err());
    }

    #[test]
    fn verifies_scrypt_auth_password_record() {
        let record =
            make_auth_password_credential("account-1", "abc123", None).expect("make record");
        assert!(is_supported_auth_password_credential(&record));
        assert!(verify_auth_password("abc123", &record).expect("verify"));
        assert!(!verify_auth_password("wrong123", &record).expect("verify wrong"));
    }

    #[test]
    fn rejects_unsupported_password_hash_parameters() {
        let mut record =
            make_auth_password_credential("account-1", "abc123", None).expect("make record");
        record.key_length = 1_000_000;
        assert!(!is_supported_auth_password_credential(&record));

        let mut record =
            make_auth_password_credential("account-1", "abc123", None).expect("make record");
        record.hash = "abcdef".to_string();
        assert!(!is_supported_auth_password_credential(&record));
    }
}
