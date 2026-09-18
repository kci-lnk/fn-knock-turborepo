//! Bounded, process-local replay protection for the internal HTTP channel.
//! Availability takes priority at capacity: evict the oldest entry. Replay
//! protection covers retained entries, not evicted entries or process restarts.
use std::{
    collections::{HashSet, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

pub(super) const TIMESTAMP_WINDOW_MS: i64 = 300_000;
const CAPACITY: usize = 131_072;
// Requests may be five minutes in the future when admitted. Retain each
// nonce past the full inclusive acceptance interval (five minutes either side).
const RETENTION: Duration = Duration::from_millis(600_001);

type Fingerprint = [u8; 32];

pub(crate) struct HmacNonceCache {
    entries: Mutex<Entries>,
    capacity: usize,
}

struct Entries {
    seen: HashSet<Fingerprint>,
    expiry: VecDeque<(Instant, Fingerprint)>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Admission {
    Accepted,
    Replay,
    Unavailable,
}

impl HmacNonceCache {
    pub(crate) fn new() -> Self {
        Self::with_capacity(CAPACITY)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(Entries {
                seen: HashSet::new(),
                expiry: VecDeque::new(),
            }),
            capacity,
        }
    }

    pub(super) fn admit(&self, nonce: &str) -> Admission {
        self.admit_with_clock(nonce, Instant::now)
    }

    #[cfg(test)]
    fn admit_at(&self, nonce: &str, now: Instant) -> Admission {
        self.admit_with_clock(nonce, || now)
    }

    fn admit_with_clock(&self, nonce: &str, clock: impl FnOnce() -> Instant) -> Admission {
        let fingerprint: Fingerprint = Sha256::digest(nonce.as_bytes()).into();
        let Ok(mut entries) = self.entries.lock() else {
            return Admission::Unavailable;
        };
        // Start retention at atomic admission, not before waiting for the lock.
        // A descheduled caller must not insert an already-expired record.
        let now = clock();
        // Clamp injected test clocks as well so the expiry queue stays ordered.
        let admitted_at = entries
            .expiry
            .back()
            .map(|(at, _)| (*at).max(now))
            .unwrap_or(now);
        while let Some((at, fingerprint)) = entries.expiry.front().copied() {
            if now.saturating_duration_since(at) < RETENTION {
                break;
            }
            entries.expiry.pop_front();
            entries.seen.remove(&fingerprint);
        }
        if entries.seen.contains(&fingerprint) {
            return Admission::Replay;
        }
        if entries.seen.len() >= self.capacity
            && let Some((_, oldest)) = entries.expiry.pop_front()
        {
            entries.seen.remove(&oldest);
        }
        entries.seen.insert(fingerprint);
        entries.expiry.push_back((admitted_at, fingerprint));
        Admission::Accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_evicts_oldest_without_blocking_new_requests() {
        let cache = HmacNonceCache::with_capacity(2);
        let start = Instant::now();
        assert_eq!(cache.admit_at("a", start), Admission::Accepted);
        assert_eq!(cache.admit_at("b", start), Admission::Accepted);
        assert_eq!(cache.admit_at("b", start), Admission::Replay);
        assert_eq!(cache.admit_at("c", start), Admission::Accepted);
        assert_eq!(cache.admit_at("b", start), Admission::Replay);
        // Intentional availability tradeoff: an evicted, still-signed nonce
        // can be admitted again until its timestamp expires.
        assert_eq!(cache.admit_at("a", start), Admission::Accepted);
        for i in 0..1_000 {
            assert_eq!(
                cache.admit_at(&format!("new-{i}"), start),
                Admission::Accepted
            );
            let entries = cache.entries.lock().unwrap();
            assert_eq!(entries.seen.len(), 2);
            assert_eq!(entries.expiry.len(), 2);
        }
        assert_eq!(
            cache.admit_at("after-expiry", start + RETENTION),
            Admission::Accepted
        );
        assert_eq!(cache.entries.lock().unwrap().seen.len(), 1);
        assert_eq!(cache.entries.lock().unwrap().expiry.len(), 1);
    }

    #[test]
    fn concurrent_duplicates_are_admitted_exactly_once() {
        let cache = HmacNonceCache::with_capacity(100);
        let start = Instant::now();
        std::thread::scope(|scope| {
            let tasks: Vec<_> = (0..32)
                .map(|_| scope.spawn(|| cache.admit_at("shared", start)))
                .collect();
            assert_eq!(
                tasks
                    .into_iter()
                    .map(|task| task.join().unwrap())
                    .filter(|result| *result == Admission::Accepted)
                    .count(),
                1
            );
        });
    }

    #[test]
    fn retention_clock_is_sampled_only_after_admission_lock() {
        let cache = HmacNonceCache::with_capacity(2);
        let start = Instant::now();
        let sampled = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            let guard = cache.entries.lock().unwrap();
            let task = scope.spawn(|| {
                cache.admit_with_clock("delayed", || {
                    // Sampling must occur while the atomic admission lock is held.
                    assert!(cache.entries.try_lock().is_err());
                    sampled.store(true, std::sync::atomic::Ordering::SeqCst);
                    start + RETENTION
                })
            });
            assert!(!sampled.load(std::sync::atomic::Ordering::SeqCst));
            drop(guard);
            assert_eq!(task.join().unwrap(), Admission::Accepted);
        });
        assert!(sampled.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(
            cache.admit_at("delayed", start + RETENTION + Duration::from_secs(1)),
            Admission::Replay
        );
    }

    #[test]
    fn expiry_keeps_the_inclusive_timestamp_boundary() {
        let cache = HmacNonceCache::with_capacity(10);
        let start = Instant::now();
        assert_eq!(cache.admit_at("edge", start), Admission::Accepted);
        assert_eq!(
            cache.admit_at("edge", start + Duration::from_secs(600)),
            Admission::Replay
        );
        assert_eq!(
            cache.admit_at("edge", start + RETENTION),
            Admission::Accepted
        );
    }
}
