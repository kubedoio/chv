# Contract: CHV → NetBox Mapping (v1)

> Version: `MAPPING_VERSION = "v1"` — carried on every projected object as the
> custom field `chv_mapping_version`. Additive-only evolution; a v2 may remap
> but must first reconcile v1 objects by external id.
> Design: `docs/design/issue-239-netbox-projection.md` · ADR-023.
> Simulator conformance: ADR-024 (issue kubedoio/chv#586).

## Source of truth

The mapping consumes:

1. The **applied architecture version** (`architecture_versions.normalized_model_json`
   — the authoritative CHVArchitecture model), and
2. Live fleet facts from `InventorySnapshot` (host CPU/memory, datastore
   kind/capacity, network bridge/vlan facts) **for enrichment only**.

Fields not present in either source are never fabricated.

## Ownership custom fields (all projected objects)

| Custom field | Value | Purpose |
|---|---|---|
| `chv_external_id` | `arch:<arch_id>:<kind>/<name>:<version>` | Idempotency match key |
| `chv_architecture_id` | architecture id | Grouping / scoping |
| `chv_managed_by` | literal `chv` | Ownership marker — the write guard |
| `chv_managed_state` | `active` \| `stale` | Retention marker |
| `chv_architecture_version` | version number | Provenance |
| `chv_mapping_version` | `v1` | Contract version |

The custom-field name prefix (`chv_`) is configurable per projection config;
the field names themselves are stable contract surface.

## Object mapping

| CHV source | NetBox object | Natural key | Notes |
|---|---|---|---|
| `servers[]` | DCIM **Device** (role `chv-node`, type `chv-host`) | `name` | `site` from config or `metadata.environment` label; CPU/memory as custom fields when live facts exist |
| `instances[]` | Virtualization **VirtualMachine** | `name` | `status`: `active` when the applied run succeeded; `staged` otherwise; `cluster`/`device` from `placement.server` when resolvable |
| `instances[].networks[]` | Virtualization **Interface** (`type: virtual`) on the VM | `name` + parent VM | `description` = CHV network name |
| `networks[]` | IPAM **Prefix** | `prefix` (=`cidr`) | `description` from network name/type |
| `networks[]` with `vlan_id` | IPAM **VLAN** | `vid` (+ group) | linked to the Prefix |
| `instances[].networks[].ip` | IPAM **IPAddress** | `address` (+ vrf when used) | assigned to the VM's Interface |
| `metadata.environment`, `metadata.labels`, `metadata.owner` | NetBox **tags** + custom fields | — | tags are `chv-<label>` slugified; owner label recorded as a custom field |

### Mapping rules

1. **Names** project verbatim; names that fail NetBox slug/charset validation
   produce a `conflict` plan entry (message explains why) — never a silent
   rename.
2. **Nullable facts stay unset** — no placeholder values.
3. **Determinism** — the plan is ordered by (kind rank, name):
   `network/vlan → prefix → ip → interface → vm → device`, then
   alphabetically by name within a rank. Dry-run output is byte-stable for
   identical inputs.
4. **Enrichment precedence** — live `InventorySnapshot` facts override
   declared `servers[].resources` when both exist and differ (live wins for
   hosts; declared wins for VMs).
5. **Secrets are excluded** — `secret_ref`, `password`, `token`,
   `ssh_keys.public_key` values and any `User.auth` material are never
   projected.
6. **Unmapped kinds** — `users`, `roles`, `instance_users`, `ssh_keys`,
   `backup_targets`, `backup_policies`, `projects`, `images`, `templates`,
   `datastores` are **not projected as NetBox objects in v1**. Datastore/
   image facts may appear as device custom fields (enrichment), nothing more.
   The exclusion list is part of the contract.

## Plan entries (dry-run and export)

The plan object carries `mapping_version`, `architecture_id`,
`architecture_version`, `retention` (the configured policy — `mark_stale` |
`delete` — recorded as plan metadata so the runner and audit trail know how
`stale` entries will be executed; entries themselves are policy-independent),
`summary`, and `entries`:

```json
{
  "action": "create | update | no_op | conflict | stale",
  "kind": "device | virtual_machine | interface | prefix | vlan | ip_address",
  "chv_resource_ref": "servers/chv-node-01",
  "netbox_natural_key": { "name": "chv-node-01" },
  "external_id": "arch:arch_01HX...:server/chv-node-01:3",
  "reason": "human-readable, secret-free explanation",
  "changes": ["memory_gb: 64 → 128"]
}
```

- `conflict` entries always carry the reason (foreign owner or name occupied)
  and the object is never written.
- `stale` entries are emitted only for NetBox objects carrying
  `chv_architecture_id` of this architecture whose CHV source disappeared.

## Ownership and collision semantics

The authoritative tree (mirrors `chv-netbox-adapter`'s `compute_plan`):

```text
lookup by chv_external_id, SAME NetBox kind as the desired object
  ├─ no same-kind match
  │     → fall through to the natural-key branch below
  ├─ more than one same-kind match (ambiguous remote state)
  │     → conflict (never write; duplicates are also exempt from stale)
  └─ exactly one same-kind match
        ├─ not chv-owned (managed_by != "chv" or mapping_version != v1)
        │     → conflict (never write)
        └─ chv-owned (managed_by == "chv" AND mapping_version == v1)
              ├─ content equal   → no_op
              └─ content differs → update
                (if the match's natural key differs from the desired one and
                 the desired natural key is occupied by another object, the
                 rename cannot proceed → conflict, never write)

not matched by external id
  ├─ natural key free             → create
  ├─ natural key occupied, foreign → conflict (never write)
  ├─ natural key occupied, chv-owned but mapping_version != v1
  │                                → conflict (never write)
  └─ natural key occupied, chv-owned object of this architecture
                                   → update (partial-failure resume / version bump)
```

The write guard is unconditional: **no request is sent that would modify an
object whose `chv_managed_by` is not `chv`**, regardless of how the lookup
matched.

## Retention

- Default (`mark_stale`): removed CHV resources get
  `chv_managed_state="stale"` (and `status` set to NetBox's decommissioning
  status for VMs/devices where supported); objects remain.
- Opt-in (`delete`): `stale` objects are deleted **only if** they still carry
  `chv_managed_by="chv"` and `chv_architecture_id` of this architecture.
- Nothing is ever deleted under the default policy.

## NetBox REST surface used (v1)

Bounded to a pinned NetBox 4.x REST contract:

- `GET/POST/PATCH /api/dcim/devices/`, `/api/virtualization/virtual-machines/`,
  `/api/virtualization/interfaces/`, `/api/ipam/prefixes/`,
  `/api/ipam/vlans/`, `/api/ipam/ip-addresses/`
- Custom-field filtering on list endpoints (`?cf_chv_external_id=…`).
- The client treats any object shape outside the contract as an error (fail
  closed), not a best-effort parse.

## Simulator conformance (ADR-024)

The dev-only simulator `crates/chv-netbox-sim` emulates **exactly the REST
surface defined by this section** — the six endpoint families, their
natural-key and custom-field query parameters, pagination semantics, and
error-body shapes. Its rules:

- Every simulator behavior must trace to this contract or to a golden
  fixture under `crates/chv-netbox-sim/tests/fixtures/`; anything the client
  does not use returns 404, as real NetBox would.
- Golden fixtures are the captured wire truth. They carry a provenance
  header and are **never hand-edited after a real capture**; they are
  refreshed only by the qualification `--record` mode against a live NetBox
  instance.
- The mapping contract is the requirements document for the simulator: when
  this contract changes, the simulator and its fixtures change in the same
  PR; when real NetBox changes, the qualification tripwire catches the
  divergence and the fixtures are re-recorded.
- The simulator's `__`-prefixed control endpoints (`__seed`, `__state`,
  `__reset`, `__faults`) are test-harness surface, not part of the NetBox
  contract, and must never be referenced by production code.
