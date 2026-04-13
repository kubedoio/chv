# Failure Matrix

## Severity levels
- `S1` — informational or self-healing expected
- `S2` — degraded service, workloads continue
- `S3` — workload impact likely
- `S4` — node service interruption or operator intervention required

| Failure case | Expected node state | New placements | Existing workloads | Immediate action | Severity |
|---|---|---:|---|---|---|
| control plane unreachable, node healthy | `Degraded` or control-plane-disconnected substate | denied | continue | preserve runtime state, buffer/report later | S2 |
| `chv-agent` restart, infra services healthy | `Degraded` briefly then recover | denied during restart | continue | reload local cache, resume reconcile | S2 |
| `chv-stord` restart, backends intact | `Degraded` | denied | continue if possible | recover service, rebuild sessions where possible | S3 |
| `chv-nwd` restart, topology preserved | `Degraded` | denied | continue with brief disturbance possible | reconstruct policy and exposure rules | S3 |
| `chv-stord` persistent failure | `Degraded` to `Failed` depending on scope | denied | affected VMs may lose storage service | escalate, block risky ops, operator action | S4 |
| `chv-nwd` persistent failure | `Degraded` to `Failed` depending on scope | denied | connectivity impact likely | deterministic rebuild attempt, operator action | S4 |
| Cloud Hypervisor process crash for one VM | VM-scoped degraded condition | allowed depending on node health | impacted VM only | restart or honor VM policy | S3 |
| host disk full in runtime paths | `Degraded` | denied | continue only if safe | block new ops, emit alert | S3 |
| host memory pressure beyond threshold | `Degraded` | denied | at risk | trigger protection policy, alert | S3 |
| host network uplink failure | `Degraded` or `Failed` | denied | connectivity loss likely | alert, preserve local runtime state | S4 |
| node reboot | `Bootstrapping` then readiness path | denied until `TenantReady` | restart per policy | reconstruct services and VMs | S3 |
| stale desired-state generation received | unchanged | unchanged | unchanged | reject request, emit event | S1 |
| certificate expiration near threshold | unchanged | unchanged | unchanged | rotate cert proactively | S1 |

## Recovery rules
- prefer deterministic reconstruction over hidden in-memory state
- never allow new placements until readiness gates close cleanly again
- preserve existing workloads where safe
- destructive recovery actions require explicit operator policy or intervention
