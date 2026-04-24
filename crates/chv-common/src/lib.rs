use std::time::{SystemTime, UNIX_EPOCH};

pub mod hypervisor;

pub mod types {
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    pub struct RequestMeta {
        pub operation_id: String,
        pub request_unix_ms: i64,
    }

    #[derive(Debug, Clone)]
    pub struct OperationId(pub String);

    #[derive(Debug, Clone)]
    pub struct VolumeId(pub String);

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BackendClass(pub String);

    #[derive(Debug, Clone)]
    pub struct BackendLocator {
        pub backend_class: String,
        pub locator: String,
        pub options: HashMap<String, String>,
    }

    #[derive(Debug, Clone, Default)]
    pub struct DevicePolicy {
        pub read_bps: u64,
        pub write_bps: u64,
        pub read_iops: u64,
        pub write_iops: u64,
        pub burst_allowed: bool,
        pub read_only: bool,
        pub no_exec: bool,
    }
}

pub fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_millis() as i64
}

/// Generate a short 8-character lowercase hex resource ID (e.g. "3f7a2bc1").
pub fn gen_short_id() -> String {
    use rand::Rng;
    let bytes: [u8; 4] = rand::rng().random();
    hex::encode(bytes)
}

/// Compute SHA-256 of `input` and return it as a lowercase hex string.
pub fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Validate that `id` contains only lowercase hex characters (a-f, 0-9).
/// Returns `true` if the id is non-empty and matches `^[a-f0-9]+$`.
pub fn validate_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_short_id_is_8_hex_chars() {
        let id = gen_short_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn gen_short_id_uniqueness() {
        let ids: std::collections::HashSet<String> = (0..1000).map(|_| gen_short_id()).collect();
        assert_eq!(ids.len(), 1000);
    }

    #[test]
    fn sha256_hex_produces_64_char_hex_string() {
        let hash = sha256_hex("chv_test_token");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sha256_hex_is_deterministic() {
        assert_eq!(sha256_hex("same"), sha256_hex("same"));
    }

    #[test]
    fn validate_id_accepts_lowercase_hex() {
        assert!(validate_id("3f7a2bc1"));
        assert!(validate_id("0000ffff"));
        assert!(validate_id("abcdef01"));
    }

    #[test]
    fn validate_id_rejects_invalid() {
        assert!(!validate_id(""));
        assert!(!validate_id("ABCDEF"));          // uppercase
        assert!(!validate_id("../etc/passwd"));   // path traversal
        assert!(!validate_id("abc xyz"));         // space
        assert!(!validate_id("g1h2i3j4"));        // non-hex letters
    }
}
