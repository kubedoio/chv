# CellHV Core Phase A2 Discovery Tooling Evidence

**Date:** 2026-07-21  
**Branch:** `agent/cellhv-core-pa-openstack-discovery`  
**Highest executed tier:** T0  
**Phase status:** blocked pending a disposable T5 OpenStack lab

## Implementation declaration

- Runtime authority remains `chv-agent`; no runtime, VM lifecycle, protocol,
  provider, systemd, or packaging behavior changed.
- Acceptance IDs in scope: `OSD-001` through `OSD-005`.
- Evidence added: a strict discovery-report schema, proposed and partial
  reports, content-addressed host/source observations, safe lab preflight and
  collection tools, negative validator tests, and CI guards.
- Explicit non-scope: production Nova code, libvirt delegation, Core runtime
  changes, Neutron/Cinder integration, migration, snapshot, and any OpenStack
  compatibility or support claim.
- Rollback: remove the Phase A2 documentation, schema, validator, lab scripts,
  tests, and their CI step. No runtime state or database migration exists.

## Acceptance disposition

| ID | Result | Evidence |
|---|---|---|
| `OSD-001` | blocked | The workspace host failed the disposable-host gate before Nova/libvirt execution. The exact first platform blocker remains unknown. |
| `OSD-002` | partial T0 | Exact Nova and libvirt source revisions identify the first configuration and QEMU-oriented assumptions. Runtime cataloguing remains T5. |
| `OSD-003` | not run | Network and storage expectations are separated in the report; neither was exercised. |
| `OSD-004` | partial T0 | The native adapter responsibility map and 12-18 engineer-week estimate are source-based and provisional. |
| `OSD-005` | blocked | Path C leads provisionally, but no integration path is selected until the OSD-001 T5 result is reviewed. |

## Verification

```text
python3 -B scripts/check-cellhv-openstack-discovery.py
python3 -B scripts/check-cellhv-openstack-discovery.py --report docs/evidence/cellhv-openstack-discovery/report.json
python3 -B -m unittest tests/test_cellhv_openstack_discovery.py tests/test_openstack_discovery_lab.py
python3 -B scripts/check-cellhv-core-architecture.py
python3 -B -m unittest tests/test_cellhv_core_architecture.py
bash -n scripts/openstack-discovery/*.sh
```

The discovery validator has negative coverage for schema conditionals,
timestamp order and the five-day limit, evidence traversal and digest
mismatch, unredacted secrets, unsupported claims, false QEMU identity, and
dishonest evidence status. Lab-tool tests cover the disposable marker, pin
requirements, isolated success, redaction/checksums, unsafe sources, and
cleanup residue.

## Residual risk and stop condition

This host has unrelated bridges, tap devices, and active virtual networking;
it lacks the disposable-host marker and OpenStack/libvirt installation. The
execution policy forbids fabricating T5 evidence or mutating non-disposable
infrastructure. Phase B remains gated because Prompt 03 requires reviewed A2
discovery evidence. The next executable action is to provision a dedicated
host, fill the immutable input manifest, run the bounded Path A probe, and
validate the resulting report before selecting a path.
