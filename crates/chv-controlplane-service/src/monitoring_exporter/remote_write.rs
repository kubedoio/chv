//! Prometheus remote-write v1 wire format for the optional external
//! export worker (ADR-027 decision 9, campaign #602 PR-8).
//!
//! The upstream schema is three tiny frozen messages
//! (`prometheus.WriteRequest`), so they are hand-defined as prost
//! structs here instead of adding a build-time codegen dependency for
//! a wire format that never changes (remote-write v1 is frozen; v2 is
//! a separate content-type and a deliberate non-goal).
//!
//! The golden-bytes test below was produced by an INDEPENDENT
//! hand-encoder (manual varint/field construction in a one-off
//! script), not by this crate — it pins the wire format, not a
//! round-trip with itself. The end-to-end compatibility proof is the
//! g5b rig against a real VictoriaMetrics binary.
//!
//! Label mapping (chv-monitoring-export contract v1):
//! - name: `chv_` + registry id with `.`→`_` (consistent with the
//!   existing `chv_*` control-plane families, e.g.
//!   `vm.cpu.capacity_ratio` → `chv_vm_cpu_capacity_ratio`);
//! - labels: `__name__`, `target_kind`, `target_id`, `source` (the
//!   store keys series by source — without the label, series from
//!   different sources would collide at the receiver), then the
//!   series' dimension map verbatim in `BTreeMap` order
//!   (deterministic bytes);
//! - timestamps: milliseconds since the epoch (the remote-write
//!   convention and the store's native unit).

use std::collections::BTreeMap;

use prost::Message;

/// `X-Prometheus-Remote-Write-Version` header value for the v1
/// protocol.
pub const REMOTE_WRITE_VERSION: &str = "0.1.0";
/// `Content-Type` for the v1 protocol body.
pub const CONTENT_TYPE: &str = "application/x-protobuf";
/// `Content-Encoding` for the v1 protocol body (snappy BLOCK
/// compression — not the framed stream format).
pub const CONTENT_ENCODING: &str = "snappy";

/// `prometheus.WriteRequest` (remote.proto), hand-defined.
#[derive(Clone, PartialEq, prost::Message)]
pub struct WriteRequest {
    #[prost(message, repeated, tag = "1")]
    pub timeseries: Vec<TimeSeries>,
}

/// `prometheus.TimeSeries`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct TimeSeries {
    #[prost(message, repeated, tag = "1")]
    pub labels: Vec<Label>,
    #[prost(message, repeated, tag = "2")]
    pub samples: Vec<Sample>,
}

/// `prometheus.Label`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Label {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub value: String,
}

/// `prometheus.Sample`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Sample {
    #[prost(double, tag = "1")]
    pub value: f64,
    #[prost(int64, tag = "2")]
    pub timestamp: i64,
}

/// One series to export, in the store's own vocabulary. The worker
/// maps `chv_monitoring_store::query::HistorySeries` to this shape;
/// keeping the renderer on an owned input keeps the wire format
/// independently testable.
#[derive(Clone, Debug)]
pub struct ExportSeries {
    pub metric_id: String,
    /// `target_kind` wire string (`node`/`vm`).
    pub target_kind: String,
    pub target_id: String,
    /// `Source::as_str()` wire string.
    pub source: String,
    pub dimensions: BTreeMap<String, String>,
    pub points: Vec<ExportPoint>,
}

#[derive(Clone, Debug)]
pub struct ExportPoint {
    pub timestamp_ms: u64,
    pub value: f64,
}

/// A rendered, compressed remote-write body plus the stats the worker
/// needs for its bounded-lag accounting.
#[derive(Clone, Debug)]
pub struct RemoteWriteBody {
    /// Snappy-compressed protobuf `WriteRequest`, ready to be the
    /// request body of a push.
    pub bytes: Vec<u8>,
    pub series_count: usize,
    pub sample_count: usize,
    /// Series skipped because their `metric_id` is outside the
    /// registry — impossible through the worker (construction
    /// validates the allowlist against `lookup`), so a non-zero value
    /// is a programming error surfaced as a defensive signal, never
    /// silent data loss.
    pub skipped_unknown_metric: usize,
    /// Oldest sample timestamp in the body. Diagnostics and test
    /// assertions only — the worker parks its cursor at PASS time by
    /// design, so this is not an input to worker accounting.
    pub oldest_timestamp_ms: Option<u64>,
    /// Newest sample timestamp in the body. Diagnostics and test
    /// assertions only — same: the cursor advance is pass-time, not
    /// newest-sample-time.
    pub newest_timestamp_ms: Option<u64>,
}

/// True when `name` is a valid Prometheus label name
/// (`[a-zA-Z_][a-zA-Z0-9_]*`). Registry dimension keys must satisfy
/// this or exported series would be rejected by conforming receivers.
pub fn is_valid_label_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Per-body stats, computed in the same pass as the encoding so they
/// count exactly what is in the rendered body (nothing else).
#[derive(Clone, Debug, Default)]
struct BodyStats {
    series_count: usize,
    sample_count: usize,
    skipped_unknown_metric: usize,
    oldest: Option<u64>,
    newest: Option<u64>,
}

/// Render the uncompressed protobuf `WriteRequest` for the given
/// series. Series without points are skipped (an empty `TimeSeries`
/// carries no information and is receiver noise); series whose
/// `metric_id` is outside the registry are counted as
/// `skipped_unknown_metric` (defensive — the worker validates its
/// allowlist at construction). Label order is deterministic
/// (`__name__`, `target_kind`, `target_id`, `source`, then dimensions
/// in `BTreeMap` order) so identical inputs produce identical bytes —
/// pinned by test.
fn encode_protobuf(series: &[ExportSeries]) -> (WriteRequest, BodyStats) {
    let mut request = WriteRequest::default();
    let mut stats = BodyStats::default();
    for s in series {
        if s.points.is_empty() {
            continue;
        }
        // The maintained name mapping lives with the registry
        // (chv-monitoring-core); unknown ids cannot get here through
        // the worker.
        let Some(metric_name) = chv_monitoring_core::registry::prometheus_name(&s.metric_id) else {
            stats.skipped_unknown_metric += 1;
            continue;
        };
        let mut labels = vec![
            Label {
                name: "__name__".to_string(),
                value: metric_name,
            },
            Label {
                name: "target_kind".to_string(),
                value: s.target_kind.clone(),
            },
            Label {
                name: "target_id".to_string(),
                value: s.target_id.clone(),
            },
            Label {
                name: "source".to_string(),
                value: s.source.clone(),
            },
        ];
        for (k, v) in &s.dimensions {
            labels.push(Label {
                name: k.clone(),
                value: v.clone(),
            });
        }
        let samples = s
            .points
            .iter()
            .map(|p| Sample {
                value: p.value,
                timestamp: p.timestamp_ms as i64,
            })
            .collect();
        request.timeseries.push(TimeSeries { labels, samples });
        stats.series_count += 1;
        stats.sample_count += s.points.len();
        for p in &s.points {
            stats.oldest = Some(match stats.oldest {
                Some(o) => o.min(p.timestamp_ms),
                None => p.timestamp_ms,
            });
            stats.newest = Some(match stats.newest {
                Some(n) => n.max(p.timestamp_ms),
                None => p.timestamp_ms,
            });
        }
    }
    (request, stats)
}

/// One remote-write push. Errors carry only a redacted
/// classification — never the destination URL or its credentials.
#[derive(Debug)]
pub struct ExportPushError(pub String);

/// Render and snappy-compress a push body from the given series.
/// Compression of a self-encoded in-memory buffer cannot fail in
/// practice; the `Result` keeps that invariant off the panic path in
/// service code (the caller logs and retries the window).
pub fn render_push(series: &[ExportSeries]) -> Result<RemoteWriteBody, ExportPushError> {
    let (request, stats) = encode_protobuf(series);
    let raw = request.encode_to_vec();
    let mut encoder = snap::raw::Encoder::new();
    let bytes = encoder.compress_vec(&raw).map_err(|error| {
        ExportPushError(format!(
            "snappy compression of the export body failed: {error}"
        ))
    })?;

    Ok(RemoteWriteBody {
        bytes,
        series_count: stats.series_count,
        sample_count: stats.sample_count,
        skipped_unknown_metric: stats.skipped_unknown_metric,
        oldest_timestamp_ms: stats.oldest,
        newest_timestamp_ms: stats.newest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_series() -> ExportSeries {
        let mut dimensions = BTreeMap::new();
        dimensions.insert("interface_id".to_string(), "eth0".to_string());
        ExportSeries {
            metric_id: "vm.net.rx_bytes_total".to_string(),
            target_kind: "vm".to_string(),
            target_id: "g5-vm".to_string(),
            source: "vmm".to_string(),
            dimensions,
            points: vec![
                ExportPoint {
                    timestamp_ms: 1_700_000_000_000,
                    value: 1024.0,
                },
                ExportPoint {
                    timestamp_ms: 1_700_000_015_000,
                    value: 2048.0,
                },
            ],
        }
    }

    #[test]
    fn metric_names_map_through_the_maintained_registry() {
        assert_eq!(
            chv_monitoring_core::registry::prometheus_name("vm.cpu.capacity_ratio").as_deref(),
            Some("chv_vm_cpu_capacity_ratio")
        );
        assert_eq!(
            chv_monitoring_core::registry::prometheus_name("node.cpu.load1").as_deref(),
            Some("chv_node_cpu_load1")
        );
        assert_eq!(
            chv_monitoring_core::registry::prometheus_name("check.duration_seconds").as_deref(),
            Some("chv_check_duration_seconds")
        );
        assert_eq!(
            chv_monitoring_core::registry::prometheus_name("not.a.registry.metric"),
            None
        );
    }

    #[test]
    fn golden_wire_bytes_match_an_independent_hand_encoding() {
        // Produced by a one-off python script that constructed the
        // protobuf by hand (manual varints, field tags, LE double) —
        // independent of this crate's encoder.
        let mut series = sample_series();
        series.metric_id = "vm.cpu.capacity_ratio".to_string();
        series.dimensions.clear();
        series.points.truncate(1);
        series.points[0].value = 0.5;
        let (request, stats) = encode_protobuf(&[series]);
        assert_eq!(stats.skipped_unknown_metric, 0);
        let raw = request.encode_to_vec();
        assert_eq!(
            raw,
            hex::decode(
                "0a6f0a250a085f5f6e616d655f5f12196368765f766d5f6370755f63617061636974795f726174696f\
                 0a110a0b7461726765745f6b696e641202766d0a120a097461726765745f6964120567352d766d\
                 0a0d0a06736f757263651203766d6d121009000000000000e03f1080d095ffbc31"
            )
            .expect("golden hex")
        );
    }

    #[test]
    fn encoded_bodies_decode_back_through_prost() {
        let body = render_push(&[sample_series()]).unwrap();
        // Snappy round-trip first: the body must be the raw BLOCK
        // format a conforming receiver decompresses.
        let mut decoder = snap::raw::Decoder::new();
        let raw = decoder
            .decompress_vec(&body.bytes)
            .expect("snappy decompress");
        let request = WriteRequest::decode(raw.as_slice()).expect("prost decode");
        assert_eq!(request.timeseries.len(), 1);
        let ts = &request.timeseries[0];
        // Label set: __name__ + identity labels + dimensions.
        let names: Vec<(&str, &str)> = ts
            .labels
            .iter()
            .map(|l| (l.name.as_str(), l.value.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("__name__", "chv_vm_net_rx_bytes_total"),
                ("target_kind", "vm"),
                ("target_id", "g5-vm"),
                ("source", "vmm"),
                ("interface_id", "eth0"),
            ]
        );
        assert_eq!(ts.samples.len(), 2);
        assert_eq!(ts.samples[0].value, 1024.0);
        assert_eq!(ts.samples[0].timestamp, 1_700_000_000_000);
        assert_eq!(ts.samples[1].value, 2048.0);
        assert_eq!(ts.samples[1].timestamp, 1_700_000_015_000);
        // Stats.
        assert_eq!(body.series_count, 1);
        assert_eq!(body.sample_count, 2);
        assert_eq!(body.oldest_timestamp_ms, Some(1_700_000_000_000));
        assert_eq!(body.newest_timestamp_ms, Some(1_700_000_015_000));
    }

    #[test]
    fn rendering_is_deterministic() {
        let a = render_push(&[sample_series()]).unwrap();
        let b = render_push(&[sample_series()]).unwrap();
        assert_eq!(a.bytes, b.bytes);
    }

    #[test]
    fn pointless_empty_series_are_skipped() {
        let mut empty = sample_series();
        empty.points.clear();
        let body = render_push(&[empty]).unwrap();
        assert_eq!(body.series_count, 0);
        assert_eq!(body.sample_count, 0);
        assert_eq!(body.oldest_timestamp_ms, None);
        // The body still decodes to a well-formed empty request.
        let mut decoder = snap::raw::Decoder::new();
        let raw = decoder.decompress_vec(&body.bytes).unwrap();
        let request = WriteRequest::decode(raw.as_slice()).unwrap();
        assert!(request.timeseries.is_empty());
    }

    #[test]
    fn every_registry_metric_and_dimension_maps_to_valid_names() {
        // The export's label/metric names are only ever derived from
        // the closed registry vocabulary — pin that EVERY registry id
        // and dimension key maps to a valid Prometheus name, so a
        // future registry addition cannot silently produce series a
        // conforming receiver would reject. (The uniqueness half of
        // this property is pinned in chv-monitoring-core next to the
        // mapping itself.)
        for def in chv_monitoring_core::registry::all() {
            let name = chv_monitoring_core::registry::prometheus_name(def.id)
                .unwrap_or_else(|| panic!("registry id {} must map", def.id));
            assert!(is_valid_label_name(&name), "{name}");
            assert!(name.starts_with("chv_"));
            for key in def.dimensions {
                assert!(is_valid_label_name(key), "{}.{key}", def.id);
            }
        }
    }

    #[test]
    fn unknown_metric_ids_are_counted_not_silently_dropped() {
        // Defensive path: the worker validates its allowlist at
        // construction, so this is unreachable in production — but if
        // it ever fires, the stat is visible rather than silent loss.
        let mut series = sample_series();
        series.metric_id = "not.a.registry.metric".to_string();
        let body = render_push(&[series, sample_series()]).unwrap();
        assert_eq!(body.skipped_unknown_metric, 1);
        assert_eq!(body.series_count, 1);
        assert_eq!(body.sample_count, 2);
    }

    #[test]
    fn label_name_validation_rejects_bad_shapes() {
        assert!(is_valid_label_name("__name__"));
        assert!(is_valid_label_name("target_id"));
        assert!(!is_valid_label_name("1abc"));
        assert!(!is_valid_label_name(""));
        assert!(!is_valid_label_name("a-b"));
        assert!(!is_valid_label_name("a.b"));
    }
}
