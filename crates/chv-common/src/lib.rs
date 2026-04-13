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
