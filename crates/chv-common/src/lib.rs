use std::time::{SystemTime, UNIX_EPOCH};

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
    }
}

pub fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis() as i64
}

/// Generate a short 8-character lowercase hex resource ID (e.g. "3f7a2bc1").
pub fn gen_short_id() -> String {
    use rand::Rng;
    let bytes: [u8; 4] = rand::rng().random();
    hex::encode(bytes)
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
}
