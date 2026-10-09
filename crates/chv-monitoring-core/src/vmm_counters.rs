//! Pinned Cloud Hypervisor v53.0 `vm.counters` parser.
//!
//! **Shape (G0b-verified, `docs/evidence/native-monitoring/g0b/`):**
//! the response is a **flat device-keyed map** — top-level keys are device
//! ids (`_disk0`, `_net1`, matching `vm.info`'s `disks[].id` /
//! `nets[].id`), each mapping field → integer counter:
//!
//! - block devices: `read_bytes`, `write_bytes`, `read_ops`, `write_ops`,
//!   `read_latency_{min,max,avg}`, `write_latency_{min,max,avg}`;
//! - NICs: `rx_bytes`, `tx_bytes`, `rx_frames`, `tx_frames`.
//!
//! There is **no** `cpus`, `net` or `block` nesting — v43.0 returns the
//! same flat shape, so the previous production parser (which read
//! `/cpus/usage/cpu_seconds` and top-level `net`/`block` objects) matched
//! neither pin and silently produced zeros. The pinned OpenAPI types the
//! response as `map<string, map<string, int64>>`; in practice latency
//! sentinels emit u64::MAX, which exceeds i64, so values are read as
//! unsigned.
//!
//! **No-data sentinels:** a field equal to `u64::MAX` means "no data"
//! (G0b fixture: `write_latency_min/max = 18446744073709551615` on a
//! read-only workload). Sentinel or missing fields are `None` —
//! unavailable, never zero. Naive passthrough of these values is what
//! fabricates giant latency spikes.

use std::collections::BTreeMap;

/// The no-data sentinel shape observed on the qualified pin.
pub const NO_DATA_SENTINEL: u64 = u64::MAX;

/// Parse failures for a `vm.counters` body.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VmmCountersError {
    #[error("body is not valid JSON: {0}")]
    InvalidJson(String),
    #[error("vm.counters body must be an object, got {0}")]
    NotAnObject(&'static str),
    #[error("device {0:?} must map field names to integers")]
    NotAnObjectField(String),
}

/// The class of a device entry, inferred from its field signature (the
/// pinned API keys by device id; the fields identify what the device is).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceClass {
    Block,
    Net,
    /// Present but carrying no recognized field (forward-compat: an
    /// unknown future device kind; never guessed into a sum).
    Unknown,
}

/// Parsed `vm.counters` response: device id → field → counter value.
/// Values that fail to read as unsigned integers are absent (forward
/// tolerance for unknown field types); callers treat absent as
/// unavailable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VmmCountersMap {
    entries: BTreeMap<String, BTreeMap<String, u64>>,
}

/// One latency group (min/max/avg, microseconds) for one device and
/// direction. `avg_us` is `None` when the VMM's truncated no-data
/// artifact makes it untrustworthy (see [`VmmCountersMap::latency`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatencyValues {
    pub min_us: u64,
    pub max_us: u64,
    pub avg_us: Option<u64>,
}

/// Per-VM sums across all devices of one class. A field is `Some` only
/// when **every** device of the class reports it as a real (non-sentinel)
/// value — one device without data makes the sum unavailable, because a
/// partial sum would understate traffic as if it were zero.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeviceCounterSums {
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
    pub rx_frames: Option<u64>,
    pub tx_frames: Option<u64>,
    pub read_bytes: Option<u64>,
    pub write_bytes: Option<u64>,
    pub read_ops: Option<u64>,
    pub write_ops: Option<u64>,
}

impl DeviceCounterSums {
    /// Whether no field carried data.
    pub fn is_empty(&self) -> bool {
        self.rx_bytes.is_none()
            && self.tx_bytes.is_none()
            && self.rx_frames.is_none()
            && self.tx_frames.is_none()
            && self.read_bytes.is_none()
            && self.write_bytes.is_none()
            && self.read_ops.is_none()
            && self.write_ops.is_none()
    }
}

impl VmmCountersMap {
    /// Parse a `vm.counters` response body from the pinned v53.0 API.
    pub fn parse(raw: &str) -> Result<Self, VmmCountersError> {
        let value: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| VmmCountersError::InvalidJson(e.to_string()))?;
        let obj = value
            .as_object()
            .ok_or(VmmCountersError::NotAnObject(match value {
                serde_json::Value::Null => "null",
                serde_json::Value::Bool(_) => "a boolean",
                serde_json::Value::Number(_) => "a number",
                serde_json::Value::String(_) => "a string",
                serde_json::Value::Array(_) => "an array",
                serde_json::Value::Object(_) => "an object (unreachable)",
            }))?;

        let mut entries = BTreeMap::new();
        for (device, fields) in obj {
            let fields_obj = fields
                .as_object()
                .ok_or_else(|| VmmCountersError::NotAnObjectField(device.clone()))?;
            let mut parsed = BTreeMap::new();
            for (field, v) in fields_obj {
                // Tolerate unknown field types (absent ⇒ unavailable);
                // only well-formed unsigned integers are data.
                if let Some(n) = v.as_u64() {
                    parsed.insert(field.clone(), n);
                }
            }
            entries.insert(device.clone(), parsed);
        }
        Ok(VmmCountersMap { entries })
    }

    /// The device ids present in the response.
    pub fn device_ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }

    /// Number of device entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the response carried no devices at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Classify a device by field signature.
    pub fn class_of(&self, device_id: &str) -> DeviceClass {
        let Some(fields) = self.entries.get(device_id) else {
            return DeviceClass::Unknown;
        };
        let has_net = fields.contains_key("rx_bytes") || fields.contains_key("tx_bytes");
        let has_block = fields.contains_key("read_bytes") || fields.contains_key("write_bytes");
        match (has_net, has_block) {
            (true, false) => DeviceClass::Net,
            (false, true) => DeviceClass::Block,
            _ => DeviceClass::Unknown,
        }
    }

    /// One device's field value: `None` when missing, non-integer, or the
    /// no-data sentinel.
    pub fn field(&self, device_id: &str, field: &str) -> Option<u64> {
        let v = *self.entries.get(device_id)?.get(field)?;
        (v != NO_DATA_SENTINEL).then_some(v)
    }

    /// One device's I/O latency group (`read_latency_*` or
    /// `write_latency_*`, microseconds). Returned as a **group** because
    /// the pinned VMM reports a no-data average as a *truncated*
    /// sentinel (`1844674407370955`, G0b fixture) rather than u64::MAX —
    /// a lone `avg` reading cannot be classified without its min/max.
    /// Rules: `None` when min or max is the sentinel (no operations of
    /// this kind occurred); the average is `Some` only when it is
    /// non-sentinel and lies within `[min, max]`.
    pub fn latency(&self, device_id: &str, write: bool) -> Option<LatencyValues> {
        let prefix = if write {
            "write_latency"
        } else {
            "read_latency"
        };
        let min = self.field(device_id, &format!("{prefix}_min"))?;
        let max = self.field(device_id, &format!("{prefix}_max"))?;
        let avg = self
            .field(device_id, &format!("{prefix}_avg"))
            .filter(|a| *a >= min && *a <= max);
        Some(LatencyValues {
            min_us: min,
            max_us: max,
            avg_us: avg,
        })
    }

    /// Sums across all block devices (`read_bytes`/`write_bytes`/
    /// `read_ops`/`write_ops`). A field is `Some` only when every block
    /// device reports it; a VM with **no** block devices yields all-`None`
    /// (unavailable — the phenomenon is not measured for this VM, which
    /// is not the same as zero traffic).
    pub fn block_sums(&self) -> DeviceCounterSums {
        let devices = self.devices_of(DeviceClass::Block);
        DeviceCounterSums {
            rx_bytes: None,
            tx_bytes: None,
            rx_frames: None,
            tx_frames: None,
            read_bytes: sum_field(&devices, "read_bytes"),
            write_bytes: sum_field(&devices, "write_bytes"),
            read_ops: sum_field(&devices, "read_ops"),
            write_ops: sum_field(&devices, "write_ops"),
        }
    }

    /// Sums across all NICs (`rx_bytes`/`tx_bytes`/`rx_frames`/
    /// `tx_frames`), same completeness rule as [`Self::block_sums`].
    pub fn net_sums(&self) -> DeviceCounterSums {
        let devices = self.devices_of(DeviceClass::Net);
        DeviceCounterSums {
            rx_bytes: sum_field(&devices, "rx_bytes"),
            tx_bytes: sum_field(&devices, "tx_bytes"),
            rx_frames: sum_field(&devices, "rx_frames"),
            tx_frames: sum_field(&devices, "tx_frames"),
            read_bytes: None,
            write_bytes: None,
            read_ops: None,
            write_ops: None,
        }
    }

    fn devices_of(&self, class: DeviceClass) -> Vec<&BTreeMap<String, u64>> {
        self.entries
            .iter()
            .filter(|(id, _)| self.class_of(id) == class)
            .map(|(_, fields)| fields)
            .collect()
    }
}

/// Sum one field across every device, or `None` when any device is missing
/// it, reports the no-data sentinel, or there are no devices at all.
/// Saturating: a multi-device sum reaching u64::MAX is beyond plausible
/// counter magnitudes and cannot be distinguished from the sentinel.
fn sum_field(devices: &[&BTreeMap<String, u64>], field: &str) -> Option<u64> {
    if devices.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    for dev in devices {
        let v = *dev.get(field)?;
        if v == NO_DATA_SENTINEL {
            return None;
        }
        total = total.saturating_add(v);
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    // G0b fixture: v53.0 vm.counters.t0.json (verbatim values).
    const V53_T0: &str = r#"{
        "_disk0": {
            "read_bytes": 147753984, "read_ops": 64306,
            "write_bytes": 0, "write_ops": 0,
            "read_latency_min": 3, "read_latency_max": 65932,
            "read_latency_avg": 19,
            "write_latency_min": 18446744073709551615,
            "write_latency_max": 18446744073709551615,
            "write_latency_avg": 1844674407370955
        },
        "_net1": { "rx_bytes": 0, "rx_frames": 0, "tx_bytes": 0, "tx_frames": 0 }
    }"#;

    // G0b fixture: v53.0 vm.counters.t1.json (10 s later; disk grew).
    const V53_T1: &str = r#"{
        "_disk0": {
            "read_bytes": 147753984, "read_ops": 65367,
            "write_bytes": 0, "write_ops": 0,
            "read_latency_min": 3, "read_latency_max": 65932,
            "read_latency_avg": 19,
            "write_latency_min": 18446744073709551615,
            "write_latency_max": 18446744073709551615,
            "write_latency_avg": 1844674407370955
        },
        "_net1": { "rx_bytes": 0, "rx_frames": 0, "tx_bytes": 0, "tx_frames": 0 }
    }"#;

    // G0b fixture: v43.0 vm.counters.v43.json — same flat shape (control).
    const V43: &str = r#"{
        "_disk0": {
            "read_bytes": 31562752, "read_ops": 61646,
            "write_bytes": 0, "write_ops": 0,
            "read_latency_min": 3, "read_latency_max": 197,
            "read_latency_avg": 5,
            "write_latency_min": 18446744073709551615,
            "write_latency_max": 18446744073709551615,
            "write_latency_avg": 1844674407370955
        },
        "_net1": { "rx_bytes": 0, "rx_frames": 0, "tx_bytes": 0, "tx_frames": 0 }
    }"#;

    #[test]
    fn parses_v53_fixture_shape() {
        let m = VmmCountersMap::parse(V53_T0).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m.class_of("_disk0"), DeviceClass::Block);
        assert_eq!(m.class_of("_net1"), DeviceClass::Net);
        assert_eq!(m.field("_disk0", "read_bytes"), Some(147_753_984));
        assert_eq!(m.field("_disk0", "read_ops"), Some(64_306));
        assert_eq!(m.field("_net1", "rx_bytes"), Some(0));
    }

    #[test]
    fn latency_sentinels_are_no_data() {
        let m = VmmCountersMap::parse(V53_T0).unwrap();
        // Write latency: min/max are u64::MAX and the avg is the
        // TRUNCATED sentinel (1844674407370955) — the whole group is
        // no-data, never a giant spike.
        assert_eq!(m.field("_disk0", "write_latency_min"), None);
        assert_eq!(m.field("_disk0", "write_latency_max"), None);
        let write = m.latency("_disk0", true);
        assert_eq!(write, None, "sentinel min/max makes the group no-data");
        // Read latency: real data parses with its average.
        let read = m.latency("_disk0", false).unwrap();
        assert_eq!(read.min_us, 3);
        assert_eq!(read.max_us, 65_932);
        assert_eq!(read.avg_us, Some(19));
    }

    #[test]
    fn truncated_avg_sentinel_is_rejected_by_the_group_rule() {
        // min/max real, avg carries the truncated sentinel shape: the
        // group rule (avg within [min,max]) rejects it.
        let raw = r#"{ "_disk0": { "read_latency_min": 3, "read_latency_max": 100, "read_latency_avg": 1844674407370955 } }"#;
        let m = VmmCountersMap::parse(raw).unwrap();
        let read = m.latency("_disk0", false).unwrap();
        assert_eq!(read.avg_us, None);
    }

    #[test]
    fn block_sums_match_fixture() {
        let m = VmmCountersMap::parse(V53_T0).unwrap();
        let s = m.block_sums();
        assert_eq!(s.read_bytes, Some(147_753_984));
        assert_eq!(s.write_bytes, Some(0));
        assert_eq!(s.read_ops, Some(64_306));
        assert_eq!(s.write_ops, Some(0));
        let n = m.net_sums();
        assert_eq!(n.rx_bytes, Some(0));
        assert_eq!(n.tx_bytes, Some(0));
        assert_eq!(n.rx_frames, Some(0));
        assert_eq!(n.tx_frames, Some(0));
    }

    #[test]
    fn t0_to_t1_delta_is_the_real_disk_activity() {
        // What the legacy path consumes: cumulative counters whose
        // positive same-epoch deltas are the real traffic.
        let t0 = VmmCountersMap::parse(V53_T0).unwrap().block_sums();
        let t1 = VmmCountersMap::parse(V53_T1).unwrap().block_sums();
        assert_eq!(t1.read_ops.unwrap() - t0.read_ops.unwrap(), 1061);
        assert_eq!(t1.read_bytes, t0.read_bytes);
    }

    #[test]
    fn v43_control_has_the_same_flat_shape() {
        let m = VmmCountersMap::parse(V43).unwrap();
        assert_eq!(m.class_of("_disk0"), DeviceClass::Block);
        assert_eq!(m.field("_disk0", "read_ops"), Some(61_646));
        // The v53 fresh-boot base matches the v43 base (deterministic
        // firmware reads) — the reset-semantics evidence.
        let fresh: &str = r#"{ "_disk0": { "read_bytes": 31562752, "read_ops": 61646, "write_bytes": 0, "write_ops": 0 }, "_net1": { "rx_bytes": 0, "rx_frames": 0, "tx_bytes": 0, "tx_frames": 0 } }"#;
        let f = VmmCountersMap::parse(fresh).unwrap();
        assert_eq!(f.field("_disk0", "read_ops"), m.field("_disk0", "read_ops"));
    }

    #[test]
    fn multiple_devices_sum_completely_or_not_at_all() {
        let raw = r#"{
            "_disk0": { "read_bytes": 100, "write_bytes": 5 },
            "_disk1": { "read_bytes": 50 }
        }"#;
        let m = VmmCountersMap::parse(raw).unwrap();
        let s = m.block_sums();
        assert_eq!(s.read_bytes, Some(150));
        // _disk1 has no write_bytes: the sum is unavailable, not 5.
        assert_eq!(s.write_bytes, None);
    }

    #[test]
    fn sentinel_in_one_device_makes_the_sum_unavailable() {
        let raw = r#"{
            "_disk0": { "read_bytes": 100 },
            "_disk1": { "read_bytes": 18446744073709551615 }
        }"#;
        let m = VmmCountersMap::parse(raw).unwrap();
        assert_eq!(m.block_sums().read_bytes, None);
    }

    #[test]
    fn no_devices_means_unavailable_not_zero() {
        let m = VmmCountersMap::parse("{}").unwrap();
        assert!(m.is_empty());
        assert!(m.block_sums().is_empty());
        assert!(m.net_sums().is_empty());
    }

    #[test]
    fn unknown_device_kind_is_not_guessed() {
        let raw = r#"{ "_rng0": { "entropy_bits": 512 } }"#;
        let m = VmmCountersMap::parse(raw).unwrap();
        assert_eq!(m.class_of("_rng0"), DeviceClass::Unknown);
        assert!(m.block_sums().is_empty());
        assert!(m.net_sums().is_empty());
        // Unknown field types are tolerated (absent ⇒ unavailable).
        assert_eq!(m.field("_rng0", "entropy_bits"), Some(512));
    }

    #[test]
    fn malformed_bodies_reject_cleanly() {
        assert!(matches!(
            VmmCountersMap::parse("not json"),
            Err(VmmCountersError::InvalidJson(_))
        ));
        assert!(matches!(
            VmmCountersMap::parse("[1,2]"),
            Err(VmmCountersError::NotAnObject(_))
        ));
        assert!(matches!(
            VmmCountersMap::parse(r#"{ "_disk0": 7 }"#),
            Err(VmmCountersError::NotAnObjectField(_))
        ));
        // Non-integer field values are absent, not errors.
        let m = VmmCountersMap::parse(r#"{ "_disk0": { "read_bytes": "many" } }"#).unwrap();
        assert_eq!(m.field("_disk0", "read_bytes"), None);
    }
}
