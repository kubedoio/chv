//! Criterion bench for the layer-1 static checker on a large topology.
//!
//! See `tests/perf_large_graph.rs` for the CI gate (assertion-based,
//! release-only). This bench is informational: it does not fail the build
//! on regression, but it gives a Criterion report (`cargo bench
//! -p chv-architecture-validate`) when investigating perf changes.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[path = "../tests/common/synth.rs"]
mod synth;

fn bench_validate_500_nodes(c: &mut Criterion) {
    // (500 servers, 50 networks, 800 instances) → 500 server nodes
    // + 50 network nodes + 800 instance nodes = 1350 total nodes,
    // and 800 NIC edges. Phase 7 D3 spec calls for the 800-edge
    // baseline; node count is a function of that and the network
    // count, not the spec's primary axis.
    let model = synth::synthesize_topology(500, 50, 800);
    c.bench_function("validate_static_checks_800_edges", |b| {
        b.iter(|| {
            let _ = chv_architecture_validate::run_static_checks(black_box(&model));
        });
    });
}

criterion_group!(benches, bench_validate_500_nodes);
criterion_main!(benches);
