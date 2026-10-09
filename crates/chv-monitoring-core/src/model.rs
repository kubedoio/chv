//! The v1 sample model (`docs/specs/contracts/chv-monitoring-metrics-v1.md`).
//!
//! A [`Sample`] is one typed, timestamped observation of a registry metric
//! for one authorized target, produced by exactly one observation layer
//! ([`Source`]). The contract's central rule is encoded in the types: a
//! sample whose [`SampleQuality`] is not [`SampleQuality::Valid`] carries no
//! value — missing data is never encoded as `0` or NaN, so a consumer can
//! never mistake a placeholder for a measurement.

use std::collections::BTreeMap;

/// What a sample observes. Target IDs are resolved against authenticated
/// inventory before collection; they are indexed storage keys, not public
/// Prometheus labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Node,
    Vm,
    Volume,
    Network,
    Check,
}

/// Which observation layer generated a number. Values are attributed only
/// to the layer that actually observed them — a cgroup reading is never
/// labelled `vmm`, a VMM-process `/proc` reading is never labelled
/// `vm_cgroup`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The node's own OS (`/proc`, `sysinfo`).
    NodeOs,
    /// Observed at the VMM layer: the Cloud Hypervisor API or the
    /// identity-fenced VMM process itself.
    Vmm,
    /// A verified runtime-owned cgroup v2 hierarchy.
    VmCgroup,
    /// `chv-stord` (or another storage provider).
    StorageProvider,
    /// `chv-nwd` (or another network provider).
    NetworkProvider,
    /// The optional in-guest `chv-monitor-agent` (ADR-026).
    GuestAgent,
    /// Computed from other valid samples at query time; never materialized
    /// at ingestion.
    Derived,
}

/// Measurement kind. Counters are monotonically increasing within one boot
/// epoch; rates are computed from positive same-epoch deltas only.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Gauge,
    Counter,
    State,
}

/// Canonical unit. Unit conversion belongs to query/UI formatting; the unit
/// never changes depending on value magnitude.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Ratio,
    Cores,
    Bytes,
    BytesPerSecond,
    Seconds,
    Count,
    Operations,
    Celsius,
    Boolean,
}

/// Truthfulness marker for a sample. `quality != valid` ⇒ `value` MUST be
/// absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleQuality {
    /// A real measurement of the registered phenomenon.
    Valid,
    /// The source needs more observations before it can compute the value
    /// (e.g. the first CPU interval, or the first of two counter reads).
    InsufficientSamples,
    /// The source layer does not implement this metric on this platform or
    /// version — permanent for this configuration, not a failure.
    Unsupported,
    /// The source exists but could not produce the value this cycle (read
    /// error, timeout, missing field).
    Unavailable,
    /// The reading was rejected as wrong (negative where impossible,
    /// non-finite, malformed).
    Invalid,
    /// The observation is too old to be presented as current.
    Stale,
}

/// A sample's value. Ratios and cores are finite f64; byte and operation
/// counters are exact unsigned integers (integer precision must survive the
/// whole pipeline — JS transports them as decimal strings beyond the
/// JS-safe range).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SampleValue {
    /// Finite, non-NaN. Gauges/ratios only.
    Float(f64),
    /// Exact counters (bytes, operations, counts).
    Integer(u64),
}

impl SampleValue {
    /// Finite f64 view of the value (integer counters convert exactly up
    /// to 2^53; beyond that callers must use [`SampleValue::Integer`]).
    pub fn as_f64(&self) -> f64 {
        match self {
            SampleValue::Float(v) => *v,
            SampleValue::Integer(v) => *v as f64,
        }
    }
}

/// Dimension validation errors. Dimensions are a bounded, registered
/// allowlist — never arbitrary labels, and never `tenant`, `project`,
/// `vm_id`, `pid`, `command_line`, `token` or `secret`.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DimensionError {
    #[error("a sample may carry at most {MAX_DIMENSIONS} dimensions, got {0}")]
    TooMany(usize),
    #[error("dimension key exceeds {MAX_KEY_BYTES} bytes: {0:?}")]
    KeyTooLong(String),
    #[error("dimension value exceeds {MAX_VALUE_BYTES} bytes: {0:?}")]
    ValueTooLong(String),
    #[error("empty dimension key")]
    EmptyKey,
}

/// Contract limits for dimensions per sample.
pub const MAX_DIMENSIONS: usize = 4;
/// Maximum dimension key length in bytes.
pub const MAX_KEY_BYTES: usize = 64;
/// Maximum dimension value length in bytes.
pub const MAX_VALUE_BYTES: usize = 128;

/// Bounded dimension map (name → value), validated on construction.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Dimensions(BTreeMap<String, String>);

impl Dimensions {
    /// An empty dimension set.
    pub fn new() -> Self {
        Dimensions(BTreeMap::new())
    }

    /// Validate and insert one dimension. Returns `Err` on any contract
    /// violation so no oversized or unbounded label can enter a sample.
    pub fn insert(&mut self, key: &str, value: &str) -> Result<(), DimensionError> {
        if key.is_empty() {
            return Err(DimensionError::EmptyKey);
        }
        if key.len() > MAX_KEY_BYTES {
            return Err(DimensionError::KeyTooLong(key.to_string()));
        }
        if value.len() > MAX_VALUE_BYTES {
            return Err(DimensionError::ValueTooLong(value.to_string()));
        }
        if self.0.len() >= MAX_DIMENSIONS && !self.0.contains_key(key) {
            return Err(DimensionError::TooMany(self.0.len() + 1));
        }
        self.0.insert(key.to_string(), value.to_string());
        Ok(())
    }

    /// Builder-style insert.
    pub fn with(mut self, key: &str, value: &str) -> Result<Self, DimensionError> {
        self.insert(key, value)?;
        Ok(self)
    }

    /// Look up a dimension value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }

    /// Number of dimensions carried.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over `(key, value)` pairs in stable order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }
}

/// The schema version of the v1 sample model. Exactly `1`.
pub const SCHEMA_VERSION: u32 = 1;

/// One typed observation of a registry metric for one target.
///
/// Construct via [`SampleBuilder`], which enforces the contract rules the
/// types cannot: metric existence and source compatibility against the
/// [`crate::registry::REGISTRY`], value absence for non-valid quality, and
/// finite/non-negative values for counter kinds.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    /// Exactly [`SCHEMA_VERSION`].
    pub schema_version: u32,
    pub target_kind: TargetKind,
    /// Indexed storage key resolved against authenticated inventory.
    pub target_id: String,
    /// Must exist in the registry allowlist.
    pub metric_id: String,
    pub source: Source,
    pub kind: MetricKind,
    pub unit: Unit,
    /// Source observation time in Unix milliseconds (the manager stamps
    /// `received_at_ms` separately and never trusts the sender for it).
    pub observed_at_ms: u64,
    /// Absent unless `quality == Valid`.
    pub value: Option<SampleValue>,
    pub quality: SampleQuality,
    pub dimensions: Dimensions,
    /// Required for counter series from a restartable source: samples from
    /// different boots are different epochs and their counters must never
    /// be subtracted.
    pub boot_id: Option<String>,
    /// Stable incarnation marker for ownership and migration fencing (e.g.
    /// a VMM process start-ticks fence). Counter deltas are only valid
    /// within one epoch.
    pub identity_epoch: Option<String>,
}

/// Contract-validating sample constructor.
#[derive(Debug)]
pub struct SampleBuilder {
    target_kind: TargetKind,
    target_id: String,
    metric_id: String,
    source: Source,
    observed_at_ms: u64,
    value: Option<SampleValue>,
    quality: SampleQuality,
    dimensions: Dimensions,
    boot_id: Option<String>,
    identity_epoch: Option<String>,
}

/// Construction/validation failures that keep malformed samples out of the
/// pipeline entirely.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SampleError {
    #[error("metric {0:?} is not in the registry")]
    UnknownMetric(String),
    #[error("source {layer:?} is not allowed for metric {metric:?}")]
    SourceNotAllowed { metric: String, layer: Source },
    #[error("quality {quality:?} must not carry a value")]
    ValueOnNonValidQuality { quality: SampleQuality },
    #[error("valid sample for metric {0:?} must carry a value")]
    MissingValue(String),
    #[error("value is not finite: {0}")]
    NotFinite(f64),
    #[error("counter value must be a non-negative integer, got {0:?}")]
    InvalidCounterValue(f64),
    #[error("valid counter sample for {0:?} must carry boot_id and identity_epoch")]
    MissingCounterEpoch(String),
    #[error(transparent)]
    Dimension(#[from] DimensionError),
}

impl SampleBuilder {
    /// Begin a sample for a registry metric. Fails if the metric is unknown.
    pub fn new(
        target_kind: TargetKind,
        target_id: impl Into<String>,
        metric_id: &str,
        source: Source,
        observed_at_ms: u64,
    ) -> Result<Self, SampleError> {
        // The registry lookup also fixes kind/unit and validates the source;
        // do that once at build() so the error carries the final context.
        let _ = crate::registry::lookup(metric_id)
            .ok_or_else(|| SampleError::UnknownMetric(metric_id.to_string()))?;
        Ok(Self {
            target_kind,
            target_id: target_id.into(),
            metric_id: metric_id.to_string(),
            source,
            observed_at_ms,
            value: None,
            quality: SampleQuality::Valid,
            dimensions: Dimensions::new(),
            boot_id: None,
            identity_epoch: None,
        })
    }

    /// Attach a value (only meaningful for a valid sample).
    pub fn value(mut self, value: SampleValue) -> Self {
        self.value = Some(value);
        self
    }

    /// Set a non-valid quality. The builder does **not** silently clear
    /// a previously set value — `build()` rejects the combination so the
    /// misuse is visible at the construction site (`quality != valid`
    /// samples carry no value).
    pub fn quality(mut self, quality: SampleQuality) -> Self {
        self.quality = quality;
        self
    }

    /// Add one dimension (bounded, validated).
    pub fn dimension(mut self, key: &str, value: &str) -> Result<Self, SampleError> {
        self.dimensions.insert(key, value)?;
        Ok(self)
    }

    /// Counter-series epoch: boot id and identity fence.
    pub fn epoch(mut self, boot_id: impl Into<String>, identity_epoch: impl Into<String>) -> Self {
        self.boot_id = Some(boot_id.into());
        self.identity_epoch = Some(identity_epoch.into());
        self
    }

    /// Finish: validate against the registry and the contract's value
    /// rules.
    pub fn build(self) -> Result<Sample, SampleError> {
        let def = crate::registry::lookup(&self.metric_id)
            .ok_or_else(|| SampleError::UnknownMetric(self.metric_id.clone()))?;

        if !def.allowed_sources.contains(&self.source) {
            return Err(SampleError::SourceNotAllowed {
                metric: self.metric_id,
                layer: self.source,
            });
        }

        if self.quality != SampleQuality::Valid {
            if self.value.is_some() {
                return Err(SampleError::ValueOnNonValidQuality {
                    quality: self.quality,
                });
            }
        } else {
            let value = self
                .value
                .ok_or_else(|| SampleError::MissingValue(self.metric_id.clone()))?;
            match value {
                SampleValue::Float(v) => {
                    if !v.is_finite() {
                        return Err(SampleError::NotFinite(v));
                    }
                    if def.kind == MetricKind::Counter && (v < 0.0 || v.fract() != 0.0) {
                        return Err(SampleError::InvalidCounterValue(v));
                    }
                }
                SampleValue::Integer(_) => {}
            }
        }

        // Counters from restartable sources require the epoch fields so a
        // reset can never be subtracted as a negative delta — enforced as
        // a construction error, not a debug assertion.
        if def.kind == MetricKind::Counter
            && self.quality == SampleQuality::Valid
            && (self.boot_id.is_none() || self.identity_epoch.is_none())
        {
            return Err(SampleError::MissingCounterEpoch(self.metric_id));
        }

        Ok(Sample {
            schema_version: SCHEMA_VERSION,
            target_kind: self.target_kind,
            target_id: self.target_id,
            metric_id: self.metric_id,
            source: self.source,
            kind: def.kind,
            unit: def.unit,
            observed_at_ms: self.observed_at_ms,
            value: self.value,
            quality: self.quality,
            dimensions: self.dimensions,
            boot_id: self.boot_id,
            identity_epoch: self.identity_epoch,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_builder(metric: &str) -> SampleBuilder {
        SampleBuilder::new(
            TargetKind::Node,
            "node-1",
            metric,
            Source::NodeOs,
            1_700_000_000_000,
        )
        .unwrap()
    }

    #[test]
    fn valid_gauge_roundtrips() {
        let s = node_builder("node.cpu.capacity_ratio")
            .value(SampleValue::Float(0.42))
            .build()
            .unwrap();
        assert_eq!(s.schema_version, 1);
        assert_eq!(s.kind, MetricKind::Gauge);
        assert_eq!(s.unit, Unit::Ratio);
        assert_eq!(s.quality, SampleQuality::Valid);
        assert_eq!(s.value, Some(SampleValue::Float(0.42)));
    }

    #[test]
    fn unknown_metric_rejected() {
        let err = SampleBuilder::new(
            TargetKind::Node,
            "node-1",
            "not.a.metric",
            Source::NodeOs,
            0,
        )
        .unwrap_err();
        assert!(matches!(err, SampleError::UnknownMetric(_)));
    }

    #[test]
    fn wrong_source_rejected() {
        // vm.cpu.cores_used allows vmm/vm_cgroup, not node_os.
        let err = SampleBuilder::new(
            TargetKind::Vm,
            "vm-1",
            "vm.cpu.cores_used",
            Source::NodeOs,
            0,
        )
        .unwrap()
        .build()
        .unwrap_err();
        assert!(matches!(err, SampleError::SourceNotAllowed { .. }));
    }

    #[test]
    fn non_valid_quality_drops_value() {
        let err = node_builder("node.cpu.capacity_ratio")
            .value(SampleValue::Float(0.5))
            .quality(SampleQuality::Unavailable)
            .build()
            .unwrap_err();
        assert!(matches!(err, SampleError::ValueOnNonValidQuality { .. }));

        // Without a value it builds and carries no value.
        let s = node_builder("node.cpu.capacity_ratio")
            .quality(SampleQuality::InsufficientSamples)
            .build()
            .unwrap();
        assert_eq!(s.value, None);
        assert_eq!(s.quality, SampleQuality::InsufficientSamples);
    }

    #[test]
    fn valid_sample_requires_value() {
        let err = node_builder("node.cpu.capacity_ratio").build().unwrap_err();
        assert!(matches!(err, SampleError::MissingValue(_)));
    }

    #[test]
    fn non_finite_rejected() {
        let err = node_builder("node.cpu.capacity_ratio")
            .value(SampleValue::Float(f64::NAN))
            .build()
            .unwrap_err();
        assert!(matches!(err, SampleError::NotFinite(_)));
        let err = node_builder("node.cpu.capacity_ratio")
            .value(SampleValue::Float(f64::INFINITY))
            .build()
            .unwrap_err();
        assert!(matches!(err, SampleError::NotFinite(_)));
    }

    #[test]
    fn counter_rejects_negative_and_fractional_float() {
        let err = node_builder("node.net.rx_bytes_total")
            .value(SampleValue::Float(-1.0))
            .build()
            .unwrap_err();
        assert!(matches!(err, SampleError::InvalidCounterValue(_)));
        let err = node_builder("node.net.rx_bytes_total")
            .value(SampleValue::Float(1.5))
            .build()
            .unwrap_err();
        assert!(matches!(err, SampleError::InvalidCounterValue(_)));
        // Exact integers are fine (counters must carry their epoch).
        node_builder("node.net.rx_bytes_total")
            .value(SampleValue::Integer(18_446_744_073_709_551_615))
            .epoch("boot-1", "iface-eth0")
            .build()
            .unwrap();
        // ...and a counter without epochs is a construction error, not a
        // silent sample that a later delta could subtract across a boot.
        let err = node_builder("node.net.rx_bytes_total")
            .value(SampleValue::Integer(1))
            .build()
            .unwrap_err();
        assert!(matches!(err, SampleError::MissingCounterEpoch(_)));
    }

    #[test]
    fn dimensions_are_bounded() {
        let mut dims = Dimensions::new();
        for i in 0..4 {
            dims.insert(&format!("d{i}"), "v").unwrap();
        }
        assert_eq!(dims.insert("d4", "v"), Err(DimensionError::TooMany(5)));
        assert_eq!(
            dims.insert("dk", &"x".repeat(129)),
            Err(DimensionError::ValueTooLong("x".repeat(129)))
        );
        let long_key = "k".repeat(65);
        assert_eq!(
            dims.insert(&long_key, "v"),
            Err(DimensionError::KeyTooLong(long_key))
        );
        // Re-inserting an existing key is an update, not a new dimension.
        let mut two = Dimensions::new();
        two.insert("a", "1").unwrap();
        two.insert("a", "2").unwrap();
        assert_eq!(two.len(), 1);
        assert_eq!(two.get("a"), Some("2"));
    }

    #[test]
    fn json_serialization_matches_contract_shape() {
        let s = SampleBuilder::new(
            TargetKind::Vm,
            "83dab870-4903-48a9-9d37-486e100ed009",
            "vm.cpu.cores_used",
            Source::VmCgroup,
            1_791_576_000_000,
        )
        .unwrap()
        .value(SampleValue::Float(1.25))
        .epoch("node-boot-id", "vmm-process-start-fence")
        .build()
        .unwrap();

        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["target_kind"], "vm");
        assert_eq!(json["metric_id"], "vm.cpu.cores_used");
        assert_eq!(json["source"], "vm_cgroup");
        assert_eq!(json["kind"], "gauge");
        assert_eq!(json["unit"], "cores");
        assert_eq!(json["observed_at_ms"], 1_791_576_000_000u64);
        assert_eq!(json["value"], 1.25);
        assert_eq!(json["quality"], "valid");
        assert_eq!(json["boot_id"], "node-boot-id");
        assert_eq!(json["identity_epoch"], "vmm-process-start-fence");
    }
}
