use scrypt::{Params as ScryptParams, scrypt};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;

use crate::{crypto_utils::random_bytes, store::AuthPasswordCredential, time_utils};

const SCRYPT_N: u32 = 16_384;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;
const SCRYPT_KEY_LENGTH: usize = 64;
const SCRYPT_SALT_HEX_LENGTH: usize = 32;
const SCRYPT_HASH_HEX_LENGTH: usize = SCRYPT_KEY_LENGTH * 2;
const DUMMY_PASSWORD_SALT_HEX: &str = "000102030405060708090a0b0c0d0e0f";
const PASSWORD_HASH_QUEUE_LIMIT: usize = 8;
const PASSWORD_HASH_QUEUE_TIMEOUT: Duration = Duration::from_secs(3);
// New records use 16-byte salts. Accept reasonably sized historical salts,
// without duplicating an arbitrarily large value restored from a backup.
const MAX_PASSWORD_SALT_BYTES: usize = 1024;
pub(crate) const MAX_AUTH_PASSWORD_BYTES: usize = 128;

#[derive(Debug, thiserror::Error)]
#[error("password hashing is busy; retry shortly")]
struct PasswordHashBusy;

pub(crate) fn is_password_hash_busy(error: &anyhow::Error) -> bool {
    error.downcast_ref::<PasswordHashBusy>().is_some()
}

pub(crate) fn password_hash_error_response(
    error: &anyhow::Error,
    message: String,
) -> axum::response::Response {
    use axum::http::{HeaderValue, StatusCode, header};
    let busy = is_password_hash_busy(error);
    let mut response = crate::response::error(
        if busy {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        },
        message,
    );
    if busy {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("3"));
    }
    response
}

struct PasswordHashPool {
    workers: Arc<Semaphore>,
    admission: Arc<Semaphore>,
    queue_timeout: Duration,
}

impl PasswordHashPool {
    fn new(workers: usize, queued: usize, queue_timeout: Duration) -> Self {
        Self {
            workers: Arc::new(Semaphore::new(workers)),
            admission: Arc::new(Semaphore::new(workers + queued)),
            queue_timeout,
        }
    }

    async fn run<T, F>(&self, work: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    {
        let admission = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| PasswordHashBusy)?;
        let worker = tokio::time::timeout(self.queue_timeout, self.workers.clone().acquire_owned())
            .await
            .map_err(|_| PasswordHashBusy)?
            .map_err(|_| PasswordHashBusy)?;
        // A cancelled HTTP request cannot cancel a running blocking closure.
        // Keep BOTH slots with the actual work, including time queued in Tokio's
        // blocking pool, so cancellation cannot admit more expensive hashes.
        tokio::task::spawn_blocking(move || {
            let (_admission, _worker) = (admission, worker);
            work()
        })
        .await?
    }
}

fn password_hash_pool() -> &'static PasswordHashPool {
    static POOL: OnceLock<PasswordHashPool> = OnceLock::new();
    POOL.get_or_init(|| {
        // Each supported scrypt hash needs 16 MiB. Leave ample headroom on
        // small/cgroup-limited appliances and cap larger hosts at two hashes.
        let (memory, _) = crate::infra::system_resources::effective_memory_bytes();
        let memory_workers = memory.map_or(1, |bytes| {
            (bytes / (256 * 1024 * 1024)).clamp(1, 2) as usize
        });
        let cpus = std::thread::available_parallelism().map_or(1, usize::from);
        PasswordHashPool::new(
            cpus.min(memory_workers),
            PASSWORD_HASH_QUEUE_LIMIT,
            PASSWORD_HASH_QUEUE_TIMEOUT,
        )
    })
}

pub(crate) async fn make_auth_password_credential(
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
    )
    .await?;
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

pub(crate) async fn verify_auth_password(
    password: &str,
    record: &AuthPasswordCredential,
) -> anyhow::Result<bool> {
    if password.len() > MAX_AUTH_PASSWORD_BYTES || record.algorithm != "scrypt" {
        return Ok(false);
    }
    let expected = derive_password_hash(
        password,
        &record.salt,
        record.n.max(2),
        record.r.max(1),
        record.p.max(1),
        record.key_length.max(1),
    )
    .await?;
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

pub(crate) async fn consume_dummy_auth_password_hash(password: &str) -> anyhow::Result<()> {
    let _ = derive_password_hash(
        password,
        DUMMY_PASSWORD_SALT_HEX,
        SCRYPT_N,
        SCRYPT_R,
        SCRYPT_P,
        SCRYPT_KEY_LENGTH,
    )
    .await?;
    Ok(())
}

pub(crate) fn validate_auth_password(password: &str) -> Result<(), &'static str> {
    if password.is_empty() {
        return Err("passwordTooShort");
    }
    if password.len() > MAX_AUTH_PASSWORD_BYTES {
        return Err("passwordTooLong");
    }
    Ok(())
}

pub(crate) async fn derive_password_hash(
    password: &str,
    salt_hex: &str,
    n: u32,
    r: u32,
    p: u32,
    key_length: usize,
) -> anyhow::Result<String> {
    anyhow::ensure!(
        password.len() <= MAX_AUTH_PASSWORD_BYTES,
        "password exceeds hashing input limit"
    );
    let params = password_hash_params(n, r, p, key_length)?;
    validate_password_hash_salt(salt_hex)?;
    let password = password.to_owned();
    let salt_hex = salt_hex.to_owned();
    password_hash_pool()
        .run(move || {
            let salt = hex::decode(salt_hex)?;
            let mut output = vec![0u8; key_length];
            scrypt(password.as_bytes(), &salt, &params, &mut output)?;
            Ok(hex::encode(output))
        })
        .await
}

fn validate_password_hash_salt(salt_hex: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        salt_hex.len() <= MAX_PASSWORD_SALT_BYTES * 2
            && salt_hex.len().is_multiple_of(2)
            && is_hex_string(salt_hex),
        "invalid or oversized password salt"
    );
    Ok(())
}

fn password_hash_params(n: u32, r: u32, p: u32, key_length: usize) -> anyhow::Result<ScryptParams> {
    anyhow::ensure!(n >= 2 && n.is_power_of_two(), "invalid scrypt N parameter");
    anyhow::ensure!(
        r > 0 && p > 0 && (1..=SCRYPT_KEY_LENGTH).contains(&key_length),
        "invalid scrypt parameters"
    );
    // Check every allocation/work factor before admitting the operation. Stored
    // records (including whole-backup restores) are not necessarily produced by
    // this version's account-creation path. Never weaken an over-budget hash by
    // clamping it; reject it. Lower-cost legacy records retain their parameters.
    let r128 = u64::from(r).checked_mul(128);
    let memory = u64::from(n)
        .checked_add(u64::from(p))
        .and_then(|value| value.checked_add(1))
        .and_then(|value| r128.and_then(|r128| value.checked_mul(r128)))
        .and_then(|value| value.checked_add(key_length as u64));
    let work = u64::from(n)
        .checked_mul(u64::from(r))
        .and_then(|value| value.checked_mul(u64::from(p)));
    const MEMORY_BUDGET: u64 =
        128 * SCRYPT_R as u64 * (SCRYPT_N as u64 + SCRYPT_P as u64 + 1) + SCRYPT_KEY_LENGTH as u64;
    const WORK_BUDGET: u64 = SCRYPT_N as u64 * SCRYPT_R as u64 * SCRYPT_P as u64;
    anyhow::ensure!(
        memory.is_some_and(|bytes| bytes <= MEMORY_BUDGET)
            && work.is_some_and(|work| work <= WORK_BUDGET),
        "scrypt parameters exceed password hashing resource budget"
    );
    Ok(ScryptParams::new(n.ilog2() as u8, r, p)?)
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
        assert!(validate_auth_password("a").is_ok());
        assert!(validate_auth_password("abcdef").is_ok());
        assert!(validate_auth_password("123456").is_ok());
        assert!(validate_auth_password("abc 123").is_ok());
        assert!(validate_auth_password("").is_err());
    }

    #[tokio::test]
    async fn verifies_scrypt_auth_password_record() {
        let record = make_auth_password_credential("account-1", "abc123", None)
            .await
            .expect("make record");
        assert!(is_supported_auth_password_credential(&record));
        assert!(
            verify_auth_password("abc123", &record)
                .await
                .expect("verify")
        );
        assert!(
            !verify_auth_password("wrong123", &record)
                .await
                .expect("verify wrong")
        );
    }

    #[tokio::test]
    async fn rejects_unsupported_password_hash_parameters() {
        let mut record = make_auth_password_credential("account-1", "abc123", None)
            .await
            .expect("make record");
        record.key_length = 1_000_000;
        assert!(!is_supported_auth_password_credential(&record));

        let mut record = make_auth_password_credential("account-1", "abc123", None)
            .await
            .expect("make record");
        record.hash = "abcdef".to_string();
        assert!(!is_supported_auth_password_credential(&record));
    }

    #[test]
    fn password_hash_resource_budget_rejects_oversized_and_invalid_records() {
        assert!(password_hash_params(SCRYPT_N, SCRYPT_R, SCRYPT_P, SCRYPT_KEY_LENGTH).is_ok());
        assert!(password_hash_params(1024, 8, 1, 32).is_ok());
        for (n, r, p, len) in [
            (32_768, 8, 1, 64),
            (16_384, 8, 2, 64),
            (16_384, 8, 1, 1_000_000),
            (16_385, 8, 1, 64),
            (2, 65_536, 1, 64),
            (1 << 31, u32::MAX, u32::MAX, 64),
        ] {
            assert!(
                password_hash_params(n, r, p, len).is_err(),
                "accepted {n}/{r}/{p}/{len}"
            );
        }
    }

    #[test]
    fn password_salt_is_bounded_before_cloning_or_decoding() {
        assert!(validate_password_hash_salt(DUMMY_PASSWORD_SALT_HEX).is_ok());
        assert!(validate_password_hash_salt(&"aB".repeat(MAX_PASSWORD_SALT_BYTES)).is_ok());
        assert!(validate_password_hash_salt(&"aa".repeat(MAX_PASSWORD_SALT_BYTES + 1)).is_err());
        assert!(validate_password_hash_salt("0").is_err());
        assert!(validate_password_hash_salt("xx").is_err());
    }

    #[tokio::test]
    async fn oversized_passwords_are_rejected_before_hash_work() {
        let password = "x".repeat(MAX_AUTH_PASSWORD_BYTES + 1);
        assert!(
            derive_password_hash(
                &password,
                DUMMY_PASSWORD_SALT_HEX,
                SCRYPT_N,
                SCRYPT_R,
                SCRYPT_P,
                SCRYPT_KEY_LENGTH
            )
            .await
            .is_err()
        );
        let mut record = make_auth_password_credential("account", "valid", None)
            .await
            .unwrap();
        // Invalid stored parameters would error if oversized verification
        // reached the KDF; input rejection must be a normal false result.
        record.n = u32::MAX;
        assert!(!verify_auth_password(&password, &record).await.unwrap());
    }

    #[tokio::test]
    async fn cancelled_queued_hash_releases_admission_without_running_work() {
        let pool = Arc::new(PasswordHashPool::new(1, 1, Duration::from_secs(10)));
        let worker = pool.workers.clone().acquire_owned().await.unwrap();
        let queued_pool = pool.clone();
        let queued = tokio::spawn(async move {
            queued_pool
                .run(|| -> anyhow::Result<()> { panic!("cancelled queued hash executed") })
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while pool.admission.available_permits() == 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        queued.abort();
        assert!(queued.await.unwrap_err().is_cancelled());
        assert_eq!(pool.admission.available_permits(), 2);
        drop(worker);
        pool.run(|| Ok(())).await.unwrap();
    }

    #[test]
    fn password_hash_overload_is_retryable() {
        let response = password_hash_error_response(&PasswordHashBusy.into(), "busy".to_string());
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "3");
    }

    #[tokio::test]
    async fn cancelled_password_request_keeps_capacity_until_blocking_work_finishes() {
        let pool = Arc::new(PasswordHashPool::new(1, 0, Duration::from_secs(1)));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task_pool = pool.clone();
        let task = tokio::spawn(async move {
            task_pool
                .run(move || {
                    let _ = started_tx.send(());
                    release_rx.recv().unwrap();
                    Ok(())
                })
                .await
        });
        // This single-thread runtime remains responsive while the closure is
        // blocked. Aborting only its async caller must not free its hash slot.
        tokio::time::timeout(Duration::from_secs(1), started_rx)
            .await
            .unwrap()
            .unwrap();
        task.abort();
        let _ = task.await;
        let rejected = pool.run(|| Ok(())).await.unwrap_err();
        assert!(is_password_hash_busy(&rejected));
        release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while pool.admission.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        pool.run(|| Ok(())).await.unwrap();
    }

    #[tokio::test]
    async fn password_hash_queue_is_bounded_and_wait_has_a_deadline() {
        let pool = Arc::new(PasswordHashPool::new(1, 1, Duration::from_millis(25)));
        let worker = pool.workers.clone().acquire_owned().await.unwrap();
        let task_pool = pool.clone();
        let queued = tokio::spawn(async move { task_pool.run(|| Ok(())).await });
        tokio::time::timeout(Duration::from_secs(1), async {
            while pool.admission.available_permits() == 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // Simulate the running request's admission independently of the held
        // worker so this test exercises only admission and queue deadlines.
        let running_admission = pool.admission.clone().acquire_owned().await.unwrap();
        assert!(is_password_hash_busy(
            &pool.run(|| Ok(())).await.unwrap_err()
        ));
        assert!(is_password_hash_busy(&queued.await.unwrap().unwrap_err()));
        drop((running_admission, worker));
        pool.run(|| Ok(())).await.unwrap();
    }
}
