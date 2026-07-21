# CellHV Core Native Local API v1

Status: Phase B local authority contract; explicit production composition available

## Boundary

`cellhv-core-api` is an HTTP/JSON transport around an injected clone of the
single `cellhv-core-operations::AuthorityHandle`. It has no SQLite, provider,
Cloud Hypervisor, control-plane, or cloud model dependency. It is intended to
run inside `chv-agent`; it is not a daemon or a second lifecycle authority.

The configured endpoint is `/run/chv/core/core-v1.sock`. `chv-agent` starts the
listener only when explicitly configured with `authority_mode = "core-native"`.
The default remains the legacy authority mode, so an upgrade does not silently
cut over production VM authority. Core-native mode does not compose the legacy
Controller, VMM, or provider stack.

## Contract

| Method | Path | Semantics |
|---|---|---|
| `GET` | `/v1/host` | Durable host identity and truthful capabilities |
| `GET` | `/v1/host/capabilities` | Capability flags; all default to false |
| `GET`, `POST` | `/v1/vms` | List definitions; asynchronously accept create |
| `GET`, `PATCH`, `DELETE` | `/v1/vms/{id}` | Inspect; asynchronously accept update/delete |
| `POST` | `/v1/vms/{id}/actions/{start,stop,reboot}` | Structured `unsupported` until an executor is wired |
| `GET` | `/v1/operations` | Ordered operation journal inspection |
| `GET` | `/v1/operations/{id}` | Operation journal entry inspection |
| `GET` | `/v1/events?after=N&limit=M` | Ordered polling; limit is 1 through 1000 |

Mutation bodies carry a caller `request_id`; Core maps it to the durable,
surface-namespaced operation ID `native:v1:{request_id}`. Every supported mutation requires an
`Idempotency-Key` header. Update and delete additionally require `If-Match`
containing exactly one quoted positive decimal resource version, for example
`"7"`. Bare integers, weak tags, lists, zero, and noncanonical leading zeroes
are rejected. A stale version returns HTTP 412. PATCH definitions carry the
next resource version (`If-Match + 1`). Accepted mutations return
HTTP 202 and the durable operation identity, disposition (`accepted` or
`replay`), and reserved resource version.

The schema rejects unknown fields through the Core domain and request types.
It contains no tenant, project, quota, scheduler, Neutron, Cinder, Kubernetes,
libvirt XML, or other cloud-platform fields.

## Local security

`bind_private` fails closed unless the socket parent already exists, is a real
directory owned by the effective service user, and has exactly mode `0700`.
It refuses to replace any existing path, binds while the directory is
inaccessible to other users, and sets the socket to mode `0600`. This owner-only policy provides an authenticated local principal
without relying on unimplemented request credentials. Production startup must
also own stale-socket cleanup explicitly after proving the prior process is
gone.

## Deferred work

- NodeCache cutover and legacy gRPC routing into this same operation service;
- deterministic OpenAPI publication/client generation decision;
- event streaming (v1 exposes deterministic polling);
- lifecycle execution and corresponding capability enablement.

The router cannot construct an `OperationService` or actor. Async handlers send
typed requests through the bounded shared authority queue and await typed
replies, preserving authority ordering without blocking Tokio workers. The
`chv-agent` composition root retains the actor owner for the whole listener
lifetime and performs ordered shutdown in explicit core-native mode.

`vm_definitions=false` currently describes executable production availability:
the writable definition journal is not an executable VM-definition backend and
no lifecycle executor or managed Cloud Hypervisor transport is composed. Its
CRUD contract is implemented, tested, and reachable in explicit core-native
mode, but this does not advertise O3K compute support and the flag must not
become true until executable definition handling is safely wired.
