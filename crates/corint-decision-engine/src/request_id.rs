//! Shared request IDs: `rq_<6 process Base62 characters>_<11 Base62 snowflake characters>`.
//!
//! Generation is local to the process. Random node IDs and random process segments
//! reduce cross-process collisions; they do not guarantee global uniqueness.

use rand::distributions::{Alphanumeric, DistString};
use rand::{rngs::OsRng, Rng};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EPOCH_MS: u64 = 1_704_067_200_000; // 2024-01-01 UTC
const MAX_TIMESTAMP: u64 = (1 << 41) - 1;
const MAX_NODE: u16 = (1 << 10) - 1;
const MAX_SEQUENCE: u16 = (1 << 12) - 1;
const BASE62: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

struct Snowflake {
    node: u16,
    last_timestamp: Option<u64>,
    sequence: u16,
}

impl Snowflake {
    fn new(node: u16) -> Self {
        assert!(node <= MAX_NODE);
        Self {
            node,
            last_timestamp: None,
            sequence: 0,
        }
    }

    // None means the caller must wait for the clock to advance. Rollback keeps
    // the previous logical timestamp and sequence instead of reusing old IDs.
    fn next_at(&mut self, timestamp: u64) -> Option<u64> {
        assert!(timestamp <= MAX_TIMESTAMP, "request ID timestamp exhausted");
        let timestamp = timestamp.max(self.last_timestamp.unwrap_or(0));
        if self.last_timestamp == Some(timestamp) {
            if self.sequence == MAX_SEQUENCE {
                return None;
            }
            self.sequence += 1;
        } else {
            self.last_timestamp = Some(timestamp);
            self.sequence = 0;
        }
        Some((timestamp << 22) | (u64::from(self.node) << 12) | u64::from(self.sequence))
    }
}

static GENERATOR: LazyLock<Mutex<Snowflake>> =
    LazyLock::new(|| Mutex::new(Snowflake::new(OsRng.gen_range(0..=MAX_NODE))));
static PROCESS_SEGMENT: LazyLock<String> =
    LazyLock::new(|| Alphanumeric.sample_string(&mut OsRng, 6));

/// Generate a 21-character, case-sensitive request ID without external services.
///
/// The process segment is sampled once on first use and shared by all threads.
/// A fresh process samples a new segment; it is not persisted. The snowflake
/// uses 41 timestamp bits, 10 process-local random node bits and 12 sequence bits.
/// Callers must store and compare the complete ID, preserving case.
///
/// # Panics
/// Panics if OS entropy is unavailable, the clock is outside the supported epoch
/// range, or generator state is poisoned; never wraps into previously used IDs.
pub fn generate_request_id() -> String {
    let process_segment = PROCESS_SEGMENT.as_str();
    let snowflake = loop {
        let mut generator = GENERATOR.lock().expect("request ID generator poisoned");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("request ID clock precedes Unix epoch")
            .as_millis();
        let timestamp = u64::try_from(timestamp)
            .ok()
            .and_then(|timestamp| timestamp.checked_sub(EPOCH_MS))
            .expect("request ID clock is outside supported epoch");
        if let Some(id) = generator.next_at(timestamp) {
            break id;
        }
        drop(generator);
        std::thread::sleep(Duration::from_millis(1));
    };
    format!("rq_{process_segment}_{}", encode_base62(snowflake))
}

fn encode_base62(mut value: u64) -> String {
    let mut output = [b'0'; 11];
    for slot in output.iter_mut().rev() {
        *slot = BASE62[(value % 62) as usize];
        value /= 62;
    }
    String::from_utf8(output.to_vec()).expect("Base62 is ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn clock_rollback_does_not_reuse_sequence() {
        let mut generator = Snowflake::new(17);
        let first = generator.next_at(100).unwrap();
        assert_eq!(generator.next_at(99), Some(first + 1));
        assert_eq!(generator.next_at(100), Some(first + 2));
        assert_eq!(generator.next_at(101), Some((101 << 22) | (17 << 12)));
    }

    #[test]
    fn sequence_exhaustion_waits_without_wrapping() {
        let mut generator = Snowflake::new(0);
        for sequence in 0..=MAX_SEQUENCE {
            assert_eq!(
                generator.next_at(10),
                Some((10 << 22) | u64::from(sequence))
            );
        }
        assert_eq!(generator.next_at(10), None);
        assert_eq!(generator.next_at(9), None);
        assert_eq!(generator.next_at(11), Some(11 << 22));
    }

    #[test]
    fn node_bits_separate_otherwise_identical_snowflakes() {
        assert_ne!(
            Snowflake::new(0).next_at(1),
            Snowflake::new(MAX_NODE).next_at(1)
        );
    }

    #[test]
    #[should_panic(expected = "request ID timestamp exhausted")]
    fn timestamp_exhaustion_never_truncates_or_wraps() {
        Snowflake::new(0).next_at(MAX_TIMESTAMP + 1);
    }

    #[test]
    fn base62_preserves_case_and_round_trips_full_snowflake_range() {
        assert_eq!(encode_base62(0), "00000000000");
        assert_eq!(encode_base62(10), "0000000000A");
        assert_eq!(encode_base62(36), "0000000000a");
        for value in [0, 61, 62, (1 << 63) - 1] {
            let encoded = encode_base62(value);
            let decoded = encoded.bytes().fold(0u64, |acc, byte| {
                acc * 62
                    + BASE62
                        .iter()
                        .position(|candidate| *candidate == byte)
                        .unwrap() as u64
            });
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn concurrent_requests_share_process_segment_and_have_distinct_snowflakes() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    (0..4_000)
                        .map(|_| generate_request_id())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut snowflakes = HashSet::new();
        let mut process_segments = HashSet::new();
        for id in threads
            .into_iter()
            .flat_map(|thread| thread.join().unwrap())
        {
            let parts: Vec<_> = id.split('_').collect();
            assert_eq!(id.len(), 21);
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0], "rq");
            assert_eq!(parts[1].len(), 6);
            assert_eq!(parts[2].len(), 11);
            assert!(parts[1]
                .bytes()
                .chain(parts[2].bytes())
                .all(|byte| BASE62.contains(&byte)));
            assert!(
                snowflakes.insert(parts[2].to_owned()),
                "snowflake reused: {id}"
            );
            process_segments.insert(parts[1].to_owned());
        }
        assert_eq!(snowflakes.len(), 32_000);
        assert_eq!(
            process_segments.len(),
            1,
            "process segment changed between requests"
        );
    }
}
