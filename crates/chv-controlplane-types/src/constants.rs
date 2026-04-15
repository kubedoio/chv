pub const STATUS_OK: &str = "ok";
pub const COMPONENT_AGENT: &str = "chv-agent";
pub const COMPONENT_STORD: &str = "chv-stord";
pub const COMPONENT_NWD: &str = "chv-nwd";
pub const COMPONENT_CHV: &str = "cloud-hypervisor";
pub const COMPONENT_HOST: &str = "host-bundle";
pub const SOURCE_ENROLLMENT: &str = "enrollment";
pub const SOURCE_INVENTORY: &str = "telemetry";
pub const SOURCE_PERIODIC: &str = "periodic_report";
pub const ALERT_STATUS_OPEN: &str = "open";

// Human summaries for gRPC responses
pub const SUMMARY_NODE_ENROLLED: &str = "node enrolled successfully";
pub const SUMMARY_CERT_ROTATED: &str = "certificate rotated successfully";
pub const SUMMARY_BOOTSTRAP_REPORTED: &str = "bootstrap result reported";
pub const SUMMARY_NODE_STATE_REPORTED: &str = "node state reported";
pub const SUMMARY_VM_STATE_REPORTED: &str = "vm state reported";
pub const SUMMARY_VOLUME_STATE_REPORTED: &str = "volume state reported";
pub const SUMMARY_NETWORK_STATE_REPORTED: &str = "network state reported";
pub const SUMMARY_EVENT_PUBLISHED: &str = "event published";
pub const SUMMARY_ALERT_PUBLISHED: &str = "alert published";
pub const SUMMARY_INVENTORY_REPORTED: &str = "inventory reported successfully";
pub const SUMMARY_VERSIONS_REPORTED: &str = "versions reported successfully";
