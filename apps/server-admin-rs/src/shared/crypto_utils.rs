use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

pub(crate) fn sha256_hex_bytes(input: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(input.as_ref()))
}

pub(crate) fn sha256_hex_str(value: &str) -> String {
    sha256_hex_bytes(value.as_bytes())
}

pub(crate) fn sha256_base64_url_no_pad(input: impl AsRef<[u8]>) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(input.as_ref()))
}

pub(crate) fn hmac_sha1_base64(key: &[u8], payload: &[u8]) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

pub(crate) fn hmac_sha256_bytes(key: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

pub(crate) fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> String {
    hex::encode(hmac_sha256_bytes(key, payload))
}

pub(crate) fn hmac_sha256_base64(key: &[u8], payload: &[u8]) -> String {
    general_purpose::STANDARD.encode(hmac_sha256_bytes(key, payload))
}

pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    rand::random::<[u8; N]>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_sha256_using_stable_encodings() {
        assert_eq!(
            sha256_hex_str("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_base64_url_no_pad("abc"),
            "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0"
        );
    }

    #[test]
    fn hmac_helpers_accept_empty_keys_and_match_known_vectors() {
        assert!(!hmac_sha256_hex(b"", b"payload").is_empty());
        assert_eq!(
            hmac_sha256_hex(b"key", b"The quick brown fox jumps over the lazy dog"),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
        assert_eq!(
            hmac_sha1_base64(b"key", b"The quick brown fox jumps over the lazy dog"),
            "3nybhbi3iqa8ino29wqQcBydtNk="
        );
    }

    #[test]
    fn random_bytes_returns_requested_size() {
        let value = random_bytes::<32>();
        assert_eq!(value.len(), 32);
    }
}
