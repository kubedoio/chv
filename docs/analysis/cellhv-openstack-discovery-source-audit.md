# CellHV OpenStack Discovery Source Audit

**Audit date:** 2026-07-21  
**Evidence tier:** T0 source inspection only  
**Nova revision:** `9d1dd3b8acd2df19d83692e0cade3911c17032b0` (`master`)  
**libvirt revision:** `624f1391ef58f59afdedb42da6635d80504e402b` (`master`)  

This audit narrows the real-lab work required by OSD-001 through OSD-005. It
does not prove that Nova can connect to or manage Cloud Hypervisor and it does
not constitute an OpenStack compatibility claim.

## Path A: unchanged Nova `LibvirtDriver`

The inspected Nova revision has no Cloud Hypervisor virtualization type:

- `nova/conf/libvirt.py:105-122` constrains `[libvirt] virt_type` to `kvm`,
  `lxc`, `qemu`, or `parallels`.
- `nova/conf/libvirt.py:123-140` permits an explicit connection URI, so
  `connection_uri=ch:///system` can be parsed independently of `virt_type`.
- `nova/virt/libvirt/driver.py:1565-1573` selects `qemu:///system` for every
  configured type other than `lxc` and `parallels` unless the URI is
  overridden.
- `nova/virt/libvirt/driver.py:825-863` performs QEMU version checks whenever
  the configured type is `kvm` or `qemu`.
- `nova/virt/libvirt/driver.py:1575-1618` only defines live-migration URIs for
  QEMU/KVM and Parallels.

The inspected libvirt revision has a real, distinct Cloud Hypervisor driver:

- `docs/drvch.rst:15-28` documents `ch:///session` and `ch:///system`.
- `docs/drvch.rst:47-56` calls the driver early-stage and limited to virtio
  devices.
- `src/ch/ch_driver.c:55-83` probes and opens the `ch:///system` state driver.
- `src/ch/ch_driver.c:2587-2605` registers the `ch` URI scheme and a state
  driver named `cloud-hypervisor`.
- `src/ch/ch_conf.h:30-31` and `src/ch/ch_conf.c:250-288` identify and validate
  the `cloud-hypervisor` binary.
- `src/ch/ch_monitor.c:740-779` constructs and daemonizes that binary with its
  API socket, while `src/ch/ch_process.c:920-990` owns the start sequence and
  VM state transition.

The last point is an architectural blocker, not merely a missing Nova option.
An unchanged Nova-to-libvirt path makes libvirt the VM process and lifecycle
authority. ADR-016 requires `chv-agent` to remain the single CellHV Core
runtime authority. Path A is therefore unacceptable for the supported CellHV
topology even if the T5 lab proves that Nova can open the URI.

The exact first runtime failure is intentionally unreported. It must come from
the disposable lab; source inspection cannot establish service ordering,
capability negotiation, generated XML, or the first exception observed by
`nova-compute`.

## Path B: generic upstream generalisation

At minimum, a generic non-QEMU libvirt backend effort would need to address:

| Area | Exact Nova location | Generic change candidate | Risk |
|---|---|---|---|
| Backend selection | `nova/conf/libvirt.py:105-140` | Represent libvirt driver type independently from the fixed four-value list. | High test-matrix expansion across driver branches. |
| Version gates | `nova/virt/libvirt/driver.py:805-863` | Query backend-specific minimum versions without treating a non-QEMU URI as QEMU/KVM. | Medium; existing assumptions are distributed. |
| URI and migration | `nova/virt/libvirt/driver.py:1565-1618` | Obtain backend URI and migration capabilities from a driver profile. | High; migration semantics differ by backend. |
| Disk model | `nova/virt/libvirt/volume/volume.py:43-57` and `nova/virt/libvirt/blockinfo.py:97-111` | Select storage driver and buses from reported capabilities. | High data-integrity and compatibility risk. |
| Guest devices | `nova/virt/libvirt/driver.py:6800-8050` | Gate consoles, guest agent, balloon, PCI, and controller generation by capabilities. | High XML and lifecycle test burden. |

These changes could be made generic in principle, but they do not solve the
authority conflict: the upstream libvirt `ch` state driver launches and owns
Cloud Hypervisor. Preserving `chv-agent` authority would require a new
delegation contract or a stateless libvirt adapter, which is no longer a small
generalisation of Nova configuration. No upstream patch is proposed in this
phase.

## Path C: native Nova driver estimate

A native Nova `ComputeDriver` would call the public or compatibility API of
`chv-agent`; it must never call Cloud Hypervisor directly. The bounded first
implementation is estimated at **12-18 engineer-weeks**, followed by a
separate T5 qualification period. The range assumes Phase B through D APIs,
recovery, network, and storage provider contracts already pass.

| Responsibility | CellHV adapter work | Reuse boundary | Initial disposition |
|---|---|---|---|
| Host resources and capabilities | Translate truthful Core capability and capacity responses. | Nova resource tracker and Placement reporting contracts. | Required. |
| Spawn and destroy | Map Nova instance UUID and request identity to one Core operation. | Nova image/cache orchestration where backend-neutral. | Required. |
| Power and `get_info` | Map durable Core desired/observed state without inventing QEMU states. | Nova power-state enums at the adapter edge. | Required. |
| Image preparation | Resolve a pinned image to a qualified raw/block attachment. | Glance download and checksum helpers. | Required. |
| Neutron VIF | Translate one qualified VIF model to the Phase D network contract. | Nova network model parsing. | Required, one model only. |
| Cinder volume | Translate one qualified block-device model to the Phase D storage contract. | Nova block-device mapping parsing. | Required, one model only. |
| Console | Expose only a Core-supported console endpoint. | Nova console authorization flow. | Required if Core advertises it. |
| Retry and restart | Persist Nova request identity as the Core idempotency key and reconcile after `nova-compute` restart. | Nova task state and retry framework. | Required. |
| Migration, resize, snapshot, evacuation, passthrough | Return explicit unsupported errors and capability flags. | None in the initial profile. | Unsupported. |

Effort ownership should be split between one senior Nova/Python engineer and
one senior Rust/virtualization engineer, with a half-time lab engineer. The
estimate includes adapter unit/functional tests and deployment packaging, but
not the Core Phase B-D implementation or broad OpenStack feature parity.

## Network and storage separation

No Neutron or Cinder behavior was exercised in this audit. Static expectations
are kept separate:

- Network: libvirt's CH example uses a virtio Ethernet interface
  (`docs/drvch.rst:68-75`). A lab must record the exact Nova VIF XML and whether
  libvirt or another process creates and owns the tap. This is security- and
  authority-sensitive and cannot be inferred from URI connectivity.
- Storage: the same example uses a raw file-backed virtio disk
  (`docs/drvch.rst:68-72`). Nova contains QEMU-oriented storage selection in
  `nova/virt/libvirt/volume/volume.py:43-57`; a separate probe must record the
  actual block mapping. Cinder remains disabled until basic compute discovery
  reaches its first result.

## Provisional decision

Path A is rejected for the CellHV supported topology because it creates a
libvirt lifecycle authority alongside `chv-agent`. Path B is not currently a
bounded upstream change for the same reason. Path C is the leading candidate,
but it is **not selected or qualified** by this T0 audit. OSD-001 requires an
exact first connection success or blocker from a disposable T5 lab before the
Phase A2 decision can close.

## Reproduction

```bash
git clone --depth 1 https://opendev.org/openstack/nova.git nova
git -C nova rev-parse HEAD
git clone --depth 1 https://gitlab.com/libvirt/libvirt.git libvirt
git -C libvirt rev-parse HEAD
```

Check out the exact revisions above before following the cited paths. Line
numbers are revision-specific.
