//! Wall-clock budget gate for layer-1 static checks on a 500-node topology.
//!
//! # Why this exists alongside `benches/large_graph.rs`
//!
//! Criterion benches don't run in CI by default and don't fail the build
//! on regression. This file is the actual perf gate: a regular `#[test]`
//! that asserts a wall-clock budget and fails the build if the budget is
//! exceeded.
//!
//! # Why `#[ignore]` in debug
//!
//! Debug builds run the validator with overflow checks, no inlining, and
//! no codegen optimisation — wall-clock numbers are wildly different from
//! release. To avoid false positives blocking local `cargo test`, the
//! gate is gated to release. CI must run:
//!
//! ```bash
//! cargo test -p chv-architecture-validate --release perf_large_graph
//! ```
//!
//! to actually exercise these tests. The acceptance gate documented in
//! `task_plan.md` (Phase 7) does exactly that.
//!
//! # Why no plan-compute gate
//!
//! `chv_architecture_reconcile::compute_diff(desired, snapshot, mode)`
//! does have a public, deterministic, no-I/O entry point that fits this
//! pattern. We deliberately do **not** add a gate for it here because
//! `chv-architecture-reconcile` would need a new `dev-dependencies`
//! entry on itself (or this test would have to live in that crate).
//! Phase 7's D3 ships the validator gate; the reconcile gate is a
//! follow-up if perf becomes a concern. See task_plan.md "Decisions
//! Made" — D3 reconcile bench was deferred at the synthesis-fn-author's
//! discretion when no obvious shared place to host it surfaced without
//! cross-crate test plumbing.

#[path = "common/synth.rs"]
mod synth;

use std::time::{Duration, Instant};

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "perf gates only fire in --release; run cargo test --release"
)]
fn validate_500_nodes_completes_under_2_seconds() {
    // Phase 7 D3 baseline: 500 servers, 50 networks, 800 instances —
    // matches the spec's 800-edge target (each instance contributes
    // one NIC edge to the graph).
    let model = synth::synthesize_topology(500, 50, 800);

    let start = Instant::now();
    let findings = chv_architecture_validate::run_static_checks(&model);
    let elapsed = start.elapsed();

    // The synthesised topology is structurally sound; if static_checks
    // emits errors, the synthesizer regressed and the timing number is
    // not meaningful. Fail loudly rather than silently passing the gate.
    let errors: Vec<_> = findings
        .iter()
        .filter(|f| {
            matches!(
                f.severity,
                chv_controlplane_types::architecture::Severity::Error
            )
        })
        .collect();
    assert!(
        errors.is_empty(),
        "synthesised 500-node topology produced unexpected validation errors \
         — synthesizer regressed; first 3: {:#?}",
        errors.iter().take(3).collect::<Vec<_>>()
    );

    println!(
        "perf: validate_static_checks(500 servers, 50 networks, 800 instances) took {:?}",
        elapsed
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "validation took {:?}, budget 2s — perf regression",
        elapsed
    );
}
