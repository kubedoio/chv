# Prompt 01 — Native measurement and source correctness (G1)

Implement [ADR-025](../../specs/adr/025-native-monitoring-architecture.md), [native monitoring spec](../../specs/component/chv-native-monitoring-spec.md) and [metric contract v1](../../specs/contracts/chv-monitoring-metrics-v1.md). Run prompt 00's current-state audit first.

## Goal

Collect accurate, read-only Linux node and Cloud Hypervisor VM metrics through the existing `chv-agent` runtime. Do **not** add a new daemon, a guest agent, or history storage in this stage. Keep lifecycle operations working unchanged.

## Tasks

1. Add a reusable bounded sampler (proposed `chv-monitoring-core` library) with traits for Linux node resources, owned VM runtime, storage provider, and network provider. Include metric registry, quality and reset semantics from v1.
2. Repair the existing `sysinfo` CPU sampling path. Retain previous snapshot across cycles; do not instantiate a fresh system and infer a valid interval from one refresh. Document CPU busy and core normalization.
3. Validate pinned Cloud Hypervisor v53.0 `/vm.counters` response using version-pinned upstream fixtures and real VMM responses. Verify actual CPU, NIC and block fields. Treat optional/missing fields as unavailable, not zero.
4. Add read-only VM cgroup v2 probes for verified runtime-owned cgroups, with PID/start-time/boot-ID/cgroup identity fencing. If ownership cannot be established, return `unsupported` and do not guess a process. Do not open arbitrary guest paths.
5. Separate host-accounted VM memory from guest-available RAM; never claim guest memory usage from VMM bytes or configuration.
6. Add source adapters to `chv-stord` and `chv-nwd` for attributable read-only capacity, throughput and health where endpoints exist. Unsupported providers must report `unsupported`; do not fake coverage.
7. Add a periodic sampling task independent of reconciliation and state reports. Bound concurrent VMM calls, timeout, queues and memory. Export sampling health and dropped measurements without VM identifiers on global Prometheus labels.
8. Capture monotonic deltas, reset markers, sample age and quality. Never regress the authority modes under ADR-016.

## Tests

- Unit: CPU first sample, counter resets, integer overflow, delayed cycle, concurrent VM stop, wrong source, unavailable VMM, negative delta, configured-vs-consumed memory.
- Integration: launch real pinned VMM on KVM, generate load, read actual counters; verify memory meaning, disk and network traffic, VMM restart, process replacement, agent restart.
- Security: malicious PID path, stale VM ownership, cross-VM identification, long/hung VMM request; no VM lifecycle or journal mutation.
- Regression: all existing Core unit/integration tests, any qualified real-host lifecycle suite relevant to the changed code.

## Gate

Provide metric-by-metric proof with source, unit, sample timestamps, actual Linux/Cloud Hypervisor counter comparisons, and exact limits. This is G1 PASS only when unsupported metrics are reported honestly and VM lifecycle stays functional under collection failure. Do not proceed to G2 with guessed counters.
