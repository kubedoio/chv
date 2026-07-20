# CellHV Core Phase A1 Baseline Evidence

**Date:** 2026-07-20  
**Branch:** `agent/cellhv-core-pa-baseline`  
**Tier:** T0

## Results

| Gate | Command | Result |
|---|---|---|
| Architecture, identity, registry, and claim guards | `python3 -B scripts/check-cellhv-core-architecture.py` | pass |
| Negative guard self-tests | `python3 -B -m unittest tests/test_cellhv_core_architecture.py` | 9 passed |
| Formatting | `cargo fmt --all -- --check` | pass |
| Workspace compile | `cargo check --workspace` | pass |
| Lints | `cargo clippy --workspace -- -D warnings` | pass |
| Workspace tests | `cargo test --workspace` | pass; three existing environment/release tests ignored |
| Draft 2020-12 schema check | `Draft202012Validator.check_schema` and validation for both checked-in documents | pass |

The ignored workspace tests are the existing release-only architecture performance test and external Ceph/iSCSI health tests. No runtime, KVM, provider, or cloud-platform qualification is claimed by these results.

## Acceptance disposition

- `AGENT-CORE-001`: pass at T0. Repository and self-tests reject a parallel binary, undeclared store/operation engine, and second process-owner map.
- `AGENT-CORE-006`: pass at T0. Packaged services are allowlisted and exactly `chv-agent.service` may start the Core runtime.
- `VMM-ID-001`: pass at T0. Active implementation and packaging reject QEMU/QMP identity while allowing storage tooling such as `qemu-img`.
- `CLAIM-001`: pass at T0. The Phase A proposed claim validates against the checked-in compatibility schema.

## Limits

Static evidence cannot prove process exclusivity on a running host, durable recovery, provider behavior, or real KVM lifecycle. Those remain T2/T3 work. The proposed tuple is explicitly not qualified, and its kernel/firmware/image pins retain named owners and deadlines.
