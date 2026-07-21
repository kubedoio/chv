# CellHV Core Current-State Inventory

**Baseline:** `main` at Phase A1, inspected 2026-07-20. **Current-state
update:** `e4448a6c`, inspected 2026-07-21. **Scope:** the detailed legacy
inventory remains the factual migration input; the update below records later
default-off Core wiring without rewriting that historical baseline as if it had
already existed.

## Post-baseline Core wiring

Commit `e4448a6c` adds an explicit `AgentAuthorityMode::CoreNative` configuration
and dispatches it in `cmd/chv-agent/src/main.rs:280-282` before NodeCache load,
enrollment, provider supervision, `VmRuntime`, or `AgentServer` construction.
`start_core_native` (`:31-51`) uses `StartupTransaction::activate_native_only`,
then `CoreRuntimeOwner::start` composes exactly one durable Core store,
`AuthorityActor`, and private native API listener under one process-lifetime
authority lease. Signal handling performs ordered listener, actor, and lease
shutdown (`:54-64`). The process acceptance harness is
`scripts/integration/core-native-agent-smoke.py`.

This is a default-off, definition-authority mode, not the completed migration.
An omitted or explicit `legacy` mode still follows the baseline path described
below and directly constructs `ProcessCloudHypervisorAdapter`, `VmRuntime`, and
`AgentServer` (`cmd/chv-agent/src/main.rs:455-468`). Native mode refuses a live
NodeCache or migration provenance; it does not import or cut over the cache.
Native and legacy requests therefore do not yet share one production operation
engine. Native create/update/delete requests are durably accepted, but native
start/stop/reboot return HTTP 422 before journaling because no Core VM executor
is wired (`crates/cellhv-core-api/src/lib.rs:282-320`). Production VM launch and
management behavior remains exclusive to the legacy path.

| Concern | Default `legacy` mode | Explicit `core-native` mode |
|---|---|---|
| Host and VM identity | Controller identity copied into NodeCache and runtime maps | Host identity and accepted VM definitions are authoritative in the Core SQLite store |
| Mutation journal | Control-plane operation repository | Node-local `OperationService` behind one `AuthorityActor` |
| Public node API | Legacy control-plane gRPC | Versioned HTTP/JSON over the private Core Unix socket |
| VM side effects | Existing `VmRuntime` and `ProcessCloudHypervisorAdapter` | None; lifecycle actions fail explicitly before journal acceptance |
| Providers | Existing stord/nwd clients and supervisors | Not constructed |
| Recovery | Existing legacy reconcile behavior; no process re-adoption | Durable definition/journal reopen only; no VM process recovery |

The modes are mutually exclusive at startup, which prevents simultaneous
control inside one process but is not the final `AGENT-CORE-002` convergence
proof. There is not yet a production path that translates legacy gRPC intent
into the same actor used by the native API.

## Runtime and crate graph

The only node VM-runtime binary is `chv-agent`: `cmd/chv-agent/Cargo.toml` declares the `chv-agent` binary and depends on `chv-agent-core` and `chv-agent-runtime-ch`. In default legacy mode, `cmd/chv-agent/src/main.rs:455-468` constructs `ProcessCloudHypervisorAdapter`, `VmRuntime`, and `AgentServer` in that process. In explicit `core-native` mode, the same binary instead constructs the Core authority owner described above. `crates/chv-agent-core/Cargo.toml` depends on the Cloud Hypervisor adapter plus the stord, nwd, and control-plane proto crates. Neither the agent crates nor binary depend on control-plane store/service, UI, or Designer crates.

`chv-stord` and `chv-nwd` are provider processes, not VM authorities. `chv-agent-core::supervisor::DaemonSupervisor` starts and monitors them (`crates/chv-agent-core/src/supervisor.rs:10-20,43-84,158-201`). Packaging contains exactly `chv-agent`, `chv-stord`, and `chv-nwd` as node binaries and services (`packaging/nfpm/chv-node.yaml:8-25`).

## Identity, desired state, observed state, and persistence

| Concern | Current owner and concrete evidence |
|---|---|
| VM identity | The control plane is authoritative. `VmDesiredStateInput` is persisted by `DesiredStateRepository::upsert_vm` (`crates/chv-controlplane-store/src/desired_state.rs:443`, type at `:914`). The agent accepts the same `vm_id` in `AgentServer::apply_vm_desired_state` (`crates/chv-agent-core/src/agent_server.rs:177-215`) and keys cache/runtime maps by it; the agent does not allocate a VM identity. |
| Desired VM state | Authoritative desired state is in the control-plane SQLite store. `Orchestrator::dispatch_operation` reads it via `build_agent_vm_spec` then sends `apply_vm_desired_state` (`crates/chv-controlplane-service/src/orchestrator.rs:434-492`). A compatibility copy is `NodeCache::vm_fragments: HashMap<String, DesiredStateFragment>` (`crates/chv-agent-core/src/cache.rs:151-177`), populated by `AgentServer::apply_vm_desired_state`. |
| Observed VM state | The agent's live view is `VmRuntime::vms`, an in-memory `HashMap<String, VmRecord>` (`crates/chv-agent-core/src/vm_runtime.rs:8-23`); lifecycle methods update string-valued `runtime_status` (`:59-120`). The control plane separately persists reports through `ObservedStateRepository::upsert_vm` (`crates/chv-controlplane-store/src/observed_state.rs:300`, input type at `:503`). |
| Node-local persistence | `NodeCache::save/load` serializes JSON at the configured `cache_path` (`crates/chv-agent-core/src/cache.rs:210-258`). It contains node/enrollment identity, generations, desired fragments, attachment IDs/handles, deferred control-plane messages, and last error (`:151-181`). It does **not** contain `VmRecord`, a process PID, an ownership marker, or an operation journal. |
| Provider persistence | stord owns a separate attachment-session SQLite database through `SessionStore` and its `sessions` table (`crates/chv-stord-core/src/store.rs:8-36`). nwd owns network topology SQLite state through `TopologyStore::new/upsert/remove/list` (`crates/chv-nwd-core/src/store.rs:7-63`). These are provider state, not second VM databases. |

State that exists only in the control plane includes the durable `operations` table, idempotency keys and statuses (`crates/chv-controlplane-store/src/operations.rs:6-63`), authoritative desired VM rows, fleet/node placement, and persisted observed projections. State unique to the node JSON cache includes enrollment certificate paths, pending outbound reports, provider attachment handles, copied desired fragments, and accepted generations.

## Operations and request path

`OperationRepository::create_or_get` persists and deduplicates operations by `idempotency_key` before dispatch (`crates/chv-controlplane-store/src/operations.rs:65-127`). `Orchestrator::dispatch_operation` resolves the agent Unix socket and maps accepted operation types to node RPCs (`crates/chv-controlplane-service/src/orchestrator.rs:434-528`). `RequestMeta.operation_id` and `desired_state_version` carry correlation/generation over the authoritative proto (`proto/controlplane/control-plane-node.proto:5-13`).

The node receives `ReconcileService` and `LifecycleService` gRPC over a Unix listener in `AgentServer::serve` (`crates/chv-agent-core/src/agent_server.rs:111-145`). Agent handlers call `VmRuntime` directly and return the incoming operation ID; there is no node-local durable operation record. Reconcile-generated IDs such as `reconcile-vm-create-{vm_id}` are transient strings (`crates/chv-agent-core/src/reconcile.rs:1258-1333`). Therefore the current implementation has one durable operation engine, but it is above the agent in the control plane and cannot provide standalone Core semantics.

## Cloud Hypervisor lifecycle, names, and restart behavior

`ProcessCloudHypervisorAdapter::create_vm` validates `/dev/kvm`, creates a process with `--api-socket`, waits for that socket, sends Cloud Hypervisor REST configuration, and retains the `tokio::process::Child` (`crates/chv-agent-runtime-ch/src/process.rs:460-579,793-804`). Subsequent start/stop/reboot calls use the Cloud Hypervisor Unix HTTP API through `ch_api_request` (`:130-216`; trait surface in `crates/chv-agent-runtime-ch/src/adapter.rs:70-177`). No QEMU or QMP path participates.

`vm_runtime_dir(base, vm_id)` is exactly `<runtime_dir>/vms/<vm_id>` (`crates/chv-agent-core/src/reconcile.rs:67-71`). `prepare_vm_resources` names the API socket `<that directory>/vm.sock` (`:827-841,991-1002`); stderr is `cloud-hypervisor.stderr.log` (`crates/chv-agent-runtime-ch/src/process.rs:521-533`). The agent API socket is `AgentConfig.socket_path`, installed as `/run/chv/agent/api.sock` by the service cleanup rule (`packaging/systemd/chv-agent.service:11-12,32-33`).

There is currently **no restart inspection or re-adoption**. Both `VmRuntime::new` and `ProcessCloudHypervisorAdapter::new` create empty maps (`crates/chv-agent-core/src/vm_runtime.rs:35-41`; `crates/chv-agent-runtime-ch/src/process.rs:89-95`). No constructor scans runtime directories, sockets, PIDs, `/proc`, or systemd units. Consequently a surviving Cloud Hypervisor process is unknown to the restarted agent.

## Storage and network preparation

`prepare_vm_resources` creates the VM runtime directory, opens each requested disk through `StordClient::open_volume_with_options`, attaches it, and converts the returned export path into `VmDiskConfig` (`crates/chv-agent-core/src/reconcile.rs:827-895`). `StorageBackend` owns open/close, attach/detach, health, resize, snapshot, clone, migration block I/O, and deletion contracts (`crates/chv-stord-backends/src/trait.rs:20-160`). Implementations are local file, LVM, iSCSI, and Ceph RBD (`crates/chv-stord-backends/src/{local,lvm,iscsi,ceph}.rs`).

For NICs, `prepare_vm_resources` derives a bridge, calls `NwdClient::ensure_network_topology`, then `attach_vm_nic`, and passes the returned TAP handle into `VmNicConfig` (`crates/chv-agent-core/src/reconcile.rs:897-983`). `NetworkServiceImpl::ensure_network_topology` validates and persists topology state (`crates/chv-nwd-core/src/handlers.rs:121-200`); `LinuxExecutor` performs Linux mutations using `ip`/`bridge` commands (`crates/chv-nwd-core/src/executor.rs:160-240`). nwd also owns DHCP/DNS via dnsmasq, nftables policy, optional eBPF, VXLAN/FDB reconciliation, and link monitoring in the correspondingly named `chv-nwd-core` modules.

## Privilege boundary

Production packaging runs `chv-agent` as user/group `chv` with supplementary `kvm`, permits `/dev/kvm`, and restricts writes to `/run/chv` and `/var/lib/chv` (`packaging/systemd/chv-agent.service:6-30`). Starting Cloud Hypervisor needs KVM access, not UID 0 in this unit. Host network creation, namespaces, TAP/bridge/VXLAN, routes, nftables, eBPF loading, and dnsmasq normally require root or Linux capabilities; the packaged nwd unit grants `CAP_NET_ADMIN` and `CAP_NET_RAW` (`packaging/systemd/chv-nwd.service:17-18`), and implementations invoke `ip`, `bridge`, `nft`, and dnsmasq (`crates/chv-nwd-core/src/executor.rs:178-240`, `firewall.rs:293-325`, `dhcp.rs:47-172`). LVM, iSCSI login, RBD mapping, and some device operations require privileges, but the packaged stord unit runs as `chv` with `NoNewPrivileges=true` and no capabilities (`packaging/systemd/chv-stord.service:7-21`). Thus those backends are implemented but not generally executable under the shipped service boundary; this is a packaging/implementation contradiction for Phase D, not evidence that stord is currently privileged.

## Reuse, extraction, and retirement

- **Reuse directly:** the `chv-agent` binary bootstrap and Unix gRPC service, `VmSpec`, `VmRuntime` facade, `CloudHypervisorAdapter`/`ProcessCloudHypervisorAdapter`, provider clients and proto contracts, structured `ChvError`, tracing/metrics, console support, and provider stores/backends.
- **Extract behind stable Core interfaces:** desired-state acceptance currently in `AgentServer`; reconciliation in `Reconciler`; in-memory `VmRuntime` state; process map/socket API in `ProcessCloudHypervisorAdapter`; and attachment orchestration in `prepare_vm_resources`. Phase B must put one durable application/operation service beneath both legacy and native APIs before any handler bypass is removed.
- **Retain as compatibility adapters:** control-plane node proto, `AgentServer` legacy gRPC handlers, and `NodeCache` importer/projection during migration.
- **Eventually retire:** `NodeCache` as VM authority input, direct handler-to-runtime mutation, control-plane-only operation authority, transient reconcile operation IDs, and silent/defaulted provider specs (for example the fallback in `agent_server.rs:322-343`). Provider daemons themselves are retained.

## Test and qualification evidence

Unit and mock coverage exists throughout the three agent crates. stord has Unix-socket and SQLite round-trip smoke tests (`crates/chv-stord-core/tests/smoke.rs:18-573`); nwd has daemon handler tests with `MockExecutor` (`crates/chv-nwd-core/tests/nwd_daemon.rs:19-519`). `.github/workflows/integration-kvm.yml` targets a manually/labeled self-hosted `chv-kvm` runner and invokes `scripts/integration/kvm-smoke.sh`, whose default VMM pin is v43.0 and which checks `/dev/kvm` and service health.

That workflow is evidence of a real-KVM-capable test path, not evidence for the Core acceptance profile: the smoke script starts the control plane, tests installation/health, and does not prove VM boot, agent-death survival, process re-adoption, host reboot, identity conflict, corruption failure, 100-cycle leaks, or manager absence. No checked-in run digest demonstrates those outcomes.

The default-off native process harness now supplies bounded evidence toward
`CORE-INSTALL-001`, durable definition identity, idempotent replay, clean/killed
agent restart, and single-process authority exclusion. It does not meet the
specified T2/T3 tiers: it uses trap executables to prove absence of VMM/provider
side effects and does not boot a guest.

Still impossible with the production composition are `AGENT-CORE-002` through
`005`; `CORE-VM-001`, `CORE-ATTACH-001`, `CORE-RECOVERY-001` and `002`,
`CORE-OPS-001`, `CORE-LEAK-001`, and `CORE-AUTH-001`; `VMM-ID-002` through `004`
after restart; optional libvirt mutation correlation; and provider/OpenStack
T4/T5 qualification. These require legacy/native authority convergence, a Core
executor, process ownership and re-adoption, qualified attachment paths, and
real disposable KVM/provider/platform labs.

## Documentation contradictions

1. `docs/ARCHITECTURE.md:53` labels SQLite desired state/operation journal in the architecture. This is now accurate for explicit `core-native` definition authority, but default legacy mode still uses JSON `NodeCache` and its control-plane operation store. The diagram does not yet express that transitional split.
2. `docs/specs/cellhv-core-foundation-spec.md` describes durable identity, recovery, and re-adoption as Core ownership. Durable identity and definition acceptance now exist in default-off native mode; VM execution, observed-state recovery, and process re-adoption remain target requirements. Legacy runtime constructors still discard runtime knowledge on restart as shown above.
3. Packaging now creates the private Core state/runtime directories and the same `chv-agent` binary can run without a manager when explicitly configured for `core-native`. The shipped default remains legacy and the node package/control-plane service relationships remain compatibility constraints; this is not yet evidence that a default package installation satisfies `CORE-INSTALL-001` at T2.
4. The acceptance specification says ambiguous workloads are preserved, but current recovery of an in-memory `Failed` record deletes/recreates it (`crates/chv-agent-core/src/reconcile.rs:1446-1457`), and unknown surviving processes are not classified. Phase C needs ownership markers and fail-closed inspection before this can meet the specification.
5. The compatibility contract's tuple permits platform integration `none` (`docs/specs/contracts/cellhv-compatibility-claims-v1.md:47`), while its allowed-label section omits `none` and lists `unsupported` (`:99-105`). The Phase A schema uses `none` because the checked-in manager-absent tuple needs that literal; architecture review must reconcile the contract before a platform claim is published.
