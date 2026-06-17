#!/usr/bin/env bash
#
# check-metric-labels.sh — Detect Prometheus alert/emitter label drift.
#
# Background: Production alert rules in monitoring/rules/*.yml reference
# metric label key=value pairs that the emitter source code never produces,
# making the alerts dead-on-arrival. The team's prior retro
# (`metric-label-drift-alert-vs-emitter`) flagged this exact failure mode,
# and CR2 in PR review #108 confirmed a recurrence.
#
# This gate parses every `metrics::counter!`, `metrics::histogram!`, and
# `metrics::gauge!` call under crates/ to learn which (metric_name, label_key)
# pairs the emitters actually produce, then walks every PromQL expression in
# monitoring/rules/*.yml looking for `chv_*{label_key="value"}` patterns. If
# any rule references a (metric_name, label_key) pair that is not in the
# emitter set, the gate fails with a precise diff report.
#
# Detection scope (current iteration):
#   - Catches: alert rules using a `chv_*` metric with a label key the
#     emitter does not set (the original CR2 drift).
#   - Does NOT catch: a label _value_ that is never emitted (e.g. an alert
#     filtering on `result="failed"` when the emitter only ever emits
#     `result="ok"` / `result="err"`). Label-value enumeration would require
#     either dataflow analysis or runtime scrape-based testing; this gate
#     is intentionally conservative and focuses on the structural mismatch
#     that has bitten the project twice.
#
# Limitations and false-positive handling:
#   - Dynamic metric names (e.g., `metrics::counter!(name)` where `name` is
#     a runtime variable) are skipped; we only resolve string literals and
#     `CHV_*` const-name references resolved against `crates/chv-observability`.
#   - The metric-name suffix `_bucket` / `_count` / `_sum` (auto-generated
#     by the histogram exporter) is stripped before comparing against the
#     emitter set.
#   - A whitelist of known external metric names lives in this script for
#     metrics produced outside the Rust workspace (e.g., `up`,
#     `http_requests_total` from middleware that uses a const, kube-state).
#
# Exit codes:
#   0 — no drift detected
#   1 — drift detected (printed to stderr)
#   2 — script invocation error (missing python3, malformed inputs)

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if ! command -v python3 >/dev/null 2>&1; then
  echo "check-metric-labels.sh: python3 is required but not installed" >&2
  exit 2
fi

python3 "$REPO_ROOT/scripts/check_metric_labels.py" "$@"
