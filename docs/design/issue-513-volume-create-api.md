# #513 design — volume-create API: the standalone `CreateVolume` surface (DP6 of #379, on the #372 DP8 freeze)

**Issue:** kubedoio/chv#513 — there is no way to create a standalone
volume (a data volume not embedded in a VM-create). VM-embedded volumes
can now carry a per-disk `storage_class` end-to-end (#379's PR 1–3:
#510/#511/#512), but every volume in existence today is born inside
`POST /v1/vms`'s transaction. The issue is the DP6 handoff from #379,
co-designed with #372's contract pass, which froze the create-surface
vocabulary (DP8) and reserved `attached_vm_id`.

**Status:** **ADOPTED 2026-10-06** — all 11 decision points below were
adopted as recommended by the maintainer, with the 4-PR decomposition
recorded in §5.1. The investigation's census was taken at main
`ce3e5b3d` (verified `git rev-parse --short HEAD`); the §2 line
references are pinned to that commit — this document ships in PR 1 of
the decomposition, which adds exactly the dispatch carrier §4 Option A
names (the proto RPC, the node-client method, the orchestrator arm,
and the agent handler) and changes no census fact below.

**Premise sharpening up front (the issue's "recommended minimal shape"
is directionally right, with two corrections):**

1. **The agent-side `CreateVolume` RPC is a dispatch carrier, not a
   journaling layer.** The issue's open question 1 ("journals through
   the operation/desired-state machinery or dispatches directly like
   the legacy A10 shape") has a one-sided answer in the evidence: the
   legacy A10 `CreateVm` RPC branch has **no production caller** — the
   production VM-create path is the BFF journaling directly
   (`POST /v1/vms` → `vms`/`vm_desired_state`/`volumes`/
   `volume_desired_state`/`operations` rows) and the orchestrator
   claiming the Accepted operation (#516's finding, verbatim). A
   volume create that dispatches directly, A10-style, would build a
   second create pipeline nothing else uses. The journaling question
   is settled by precedent; the *dispatch* question is the live one
   (§4, DP2).
2. **A journal-only create (row + desired state, no provisioning) is
   not viable** — the attach path cannot materialize a backing store
   (§2.4). The create must dispatch a provisioning open, or the
   volume it mints is permanently un-attachable. This is the finding
   the issue does not anticipate.

---

## 1. Problem statement

Three gaps stack, and (as in #379) they are not one problem:

1. **No create surface.** No BFF route (`crates/chv-webui-bff/src/
   router.rs:162-165` viewer list/get, `:299-316` operator
   mutate/snapshot/restore-snapshot/delete-snapshot/clone — no
   create), no chvctl command (`cmd/chvctl/src/commands/volume.rs:8-27`
   — List/Snapshot/Clone only), no node RPC
   (`proto/controlplane/control-plane-node.proto:480-514` —
   CreateVm/Attach/Detach/Resize/Snapshot/Restore/DeleteSnapshot/Clone;
   no CreateVolume), no UI create surface (`ui/src/routes/volumes/
   +page.svelte:87-93` — an "Allocate Block" button with **no click
   handler**, a dead affordance). The CLI spec records the gap
   (`docs/specs/ops/chvctl-cli-spec.md:44`: "Volume create/show/delete
   are not implemented; the volume-create API is tracked in #513").
2. **No provisioning path for standalone volumes.** Backing stores
   materialize create-on-open *inside VM preparation* — the agent's
   `prepare_vm_resources` opens each disk with a
   `size_bytes` option (`crates/chv-agent-core/src/reconcile.rs:
   1172-1175`) and the local backend creates the sparse file
   (`crates/chv-stord-backends/src/local.rs:336-354`), LVM the LV
   (`crates/chv-stord-backends/src/lvm.rs:115-157`). Nothing opens a
   volume that has no VM, and (§2.4) the attach path cannot either.
3. **The contract is frozen but unexercised.** #372's DP8 froze the
   create-surface names (`docs/design/issue-372-chvctl-bff-contract-
   drift.md:413-446`) and reserved the `volume create` harness slot
   (`:493-494`); chvctl's contract harness is at 39 rows, zero
   pinned-broken (`cmd/chvctl/tests/contract.rs`). The freeze is a
   promise about names — this design must keep it and add its row.

The question for the maintainer is fourfold: (i) which layer journals
the create intent; (ii) which carrier provisions the backing store on
the node; (iii) whether the create accepts attachment at create time
(the reserved `attached_vm_id`); (iv) how the #378/#386/#516
disciplines (accept-time rejection, ownership, capability check)
carry onto the new surface — each with a recommendation below (§5).

## 2. Ground truth at `ce3e5b3d`

### 2.1 What the store already supports (the schema is ready)

The `volumes` table (`cmd/chv-controlplane/migrations/0001_initial.sql:
122-131`) has every column a standalone create needs — `volume_id`,
`node_id` (nullable, `ON DELETE SET NULL`), `display_name NOT NULL`,
`capacity_bytes NOT NULL`, `volume_kind`, `storage_class` (nullable) —
plus `owner_id` (migration `0027_owner_id.sql:4`, indexed). The
`volume_desired_state` table (`0001:133-148`) models unattached
volumes natively: `attached_vm_id` is nullable with
`ON DELETE SET NULL`, alongside `device_name`, `read_only`,
`attachment_mode`, `requested_by`. **A standalone create is pure
INSERTs at the schema level — no migration is needed.** The
fragment-level model agrees: the CP-side `VolumeSpec`
(`crates/chv-controlplane-types/src/fragment.rs:42-55`) already
carries `capacity_bytes`, `volume_kind`, `storage_class`, and an
optional `attached_vm_id` — the typed shape of exactly this create.

Precedent for each INSERT (the VM-create transaction,
`crates/chv-webui-bff/src/handlers/vms.rs:661-692`): volumes row with
`owner_id = claims.sub` and `storage_class` NULL-when-unnamed; VDS row
with `desired_status 'Pending'`, `requested_by`, `attached_vm_id`
(the VM's id there; NULL here). The store's `upsert_volume`
(`crates/chv-controlplane-store/src/desired_state.rs:618-667`) is the
fragment/clone writer — COALESCE-preserving on owner — and
`materialize_clone_target` (`:824-939`) is the strict-insert
transactional template should the create want one.

### 2.2 Consumers: who wants a create, and what they can call today

| Consumer | Surface today | Wants |
|---|---|---|
| chvctl | `volume list/snapshot/clone` (`volume.rs:8-27`); the phantom `volume create/show/delete` were **removed from the spec's truth** in the #372 pass (`cli-spec.md:44`) | `volume create` — the DP8 freeze names it; the harness slot is reserved (`issue-372 design §6:493-494`) |
| UI | list page with a **dead "Allocate Block" button** (`ui/src/routes/volumes/+page.svelte:87-93` — no `onclick`); detail page has an attach modal only (`ui/src/routes/volumes/[id]/+page.svelte:29-30,52-53,61-75`, filtering VMs on the volume's node, `:73-75`) | the button is the create affordance, waiting for a route |
| templates | VM-embedded only (`handlers/templates.rs:419-433`) — no standalone-volume template concept exists | out of scope (§8) |
| BFF tests | `tests/volume_snapshot_clone.rs` (the #373 pins + the seeding shape every harness row reuses, `:241`) | the create row template exists |

Two adjacent display findings on the UI list page (recorded, not
load-bearing here): the "Storage Driver" column reads `item.backend`
with a hard-coded fallback `'LOCAL_LVM'`
(`+page.svelte:58,78`) — but the BFF's volume list **does not serve
`storage_class` at all** (`handlers/volumes.rs:71-86`; only the detail
row surfaces it, `:135`, `:202`). That is #372's §2.7(a) display-drift
class on the volume list — the create PR that touches this page should
fix the column against the real field (DP10).

### 2.3 The journaling precedent — BFF-direct is the production create path

`POST /v1/vms` (`handlers/vms.rs:312-879`) journals everything itself:
`BEGIN IMMEDIATE` (`:551-555`), quota inside the tx (`:559-567`),
id-minting (`chv_common::gen_short_id`, `:569-570`), the five INSERTs
(vms `:575`, VDS `:590`, volumes `:661`, volume VDS `:680`,
operations `'CreateVm'`/`'Accepted'` `:848-862` with idempotency key
`create-vm-{vm_id}`), and the `{accepted, task_id, vm_id, summary,
next_refresh_path}` response (`:872-878`). The CP lifecycle's
`create_vm` RPC (`crates/chv-controlplane-service/src/lifecycle.rs:
663-729`) implements the same journaling behind an RPC — and has **no
production caller** (#516's finding; the same reason the class-capability
check had to be wired into the BFF handler, `vms.rs:536-545`). The
#516-shared accept-time check composition —
`NodeRepository::node_storage_class_rejection`
(`crates/chv-controlplane-store/src/nodes.rs:410`) — exists precisely
so a BFF-direct surface can run the identical decision the lifecycle
RPC runs (`lifecycle.rs:417-438`).

The orchestrator then claims Accepted operations and dispatches by
`operation_type` (`crates/chv-controlplane-service/src/orchestrator.rs:
772+`): `"create" | "CreateVm"` → `apply_vm_desired_state` (`:778-791`),
`"AttachVolume"` → `node_client.attach_volume` with the volume's class
resolved in the claim query (`:882-899`), `"CloneVolume"` →
`node_client.clone_volume` (`:976+`). An unknown `operation_type`
fails the operation with `unsupported operation_type for dispatch`
(`:1163`) — sequencing note for §7: a journaled `CreateVolume` with no
dispatch arm does not sit idle; it is actively Failed.

### 2.4 The provisioning gap — journal-only is not viable (the load-bearing finding)

The attach path **cannot materialize a never-provisioned volume**:

- The CP's attach dispatch carries no size: `node_client.attach_volume`
  (`node_client.rs:650-671`) and the BFF's attach mutation send only
  the class-carrying `volume_spec_json`
  (`crates/chv-controlplane-service/src/bff_mutations.rs:433-447`).
- The agent's `open_and_attach_volume` opens with **no options**
  (`crates/chv-agent-core/src/agent_server.rs:136-139`) — no
  `size_bytes`.
- The local backend's create-on-open fires only when the open carries
  a size (`local.rs:336-354`, fed by the `open_options` map the VM
  path builds, `reconcile.rs:1172-1175`); an LVM open of an absent LV
  without `size_bytes` is **rejected** (`lvm.rs:115-157`;
  `docs/OPERATIONS.md:410-414`: "an open of an absent LV without a
  size is rejected … and so is any `seed_from`").

So a create that only journals rows mints a volume whose first attach
fails at the stord open — an accepted-then-silently-failed UX, the
exact shape #378 was filed to kill. **The create must dispatch a
provisioning open** (size + class, no VM attach), or teach attach to
carry size (rejected: it mutates the attach contract for every
existing volume to fix a new surface's gap).

Two carriers exist for that dispatch:

- **A new `CreateVolume` node RPC** (#379 DP6's recommended shape,
  `issue-379-storage-class-dispatch.md:508-516`): mirrors
  `CreateVmRequest` (`control-plane-node.proto:196-200`) with a
  `VolumeMutationSpec` (`:190-194`) whose `volume_spec_json` carries
  `{size_bytes, backend_class}`; the agent handler is
  `prepare_vm_resources`' open minus the attach —
  `open_volume_with_options` with the size option and #379 DP5
  class-dependent locator shaping (local: the RELATIVE
  `{volume_id}.img` locator — the A2 re-attach shape
  (`reconcile.rs:988-990`) and the legacy A10 `CreateVm` branch's
  default (`agent_server.rs:1120-1127`), i.e.
  `{runtime_dir}/{volume_id}.img`, NOT A1's vm-id-nested
  `{runtime_dir}/{vm_id}/{volume_id}.img` (`reconcile.rs:1143,1187` —
  a different file, and NOT the A4 attach RPC's bare `{volume_id}`
  default either); LVM: the `/dev/mapper/{vg}-{vid}` token). The
  legacy A10 `CreateVm` branch (`agent_server.rs:1001-1120`) is the
  recorded mirror surface.
  **Locator guard for the attach follow-up (DP4/§8):** a standalone
  volume created by this carrier must later be opened by its attach
  path with the CARRIER's relative `{volume_id}.img` locator — do not
  route a standalone volume's attach through A1's vm-dir-nested
  locator or the A4 attach RPC's bare-id default, either of which
  would create-on-open a second default-size file and permanently
  orphan the file this carrier minted (the exact un-attachable-volume
  failure §2.4 exists to kill).
- **The fragment channel** (`ApplyVolumeDesiredState`,
  `node_client.rs:273-319`): no proto change; the CP-side typed
  `VolumeSpec` already models the create (`fragment.rs:42-55`). But
  the agent's fragment handler today (i) only opens+attaches when the
  *raw* spec JSON has a `"vm_id"` key (`agent_server.rs:584-591`) — a
  key that **drifts from the typed spec's `attached_vm_id`**
  (`fragment.rs:48`; the CP-side writer at
  `chv-controlplane-service/src/reconcile.rs:466-478` journals
  `attached_vm_id`) — (ii) does nothing for standalone specs (no
  open), and (iii) fails closed wholesale in core-managed mode
  (`agent_server.rs:540-544`). The channel is otherwise unused in
  production (only the migration dirty-tracking cleanup pushes
  fragments, `crates/chv-controlplane-service/src/migration.rs:
  918-935`) — available, but every fact above is a change, not a
  reuse.

### 2.5 The disciplines that must carry (already-built, one call each)

- **Vocabulary (#379 DP3):** the single shared list
  `chv_hypervisor_api::resources::BACKEND_CLASSES = ["local",
  "iscsi", "ceph", "lvm"]` (`crates/chv-hypervisor-api/src/resources.rs:
  44-52`) — never a local copy; unknown class → 400
  (`vms.rs:497-515` is the exact arm to mirror).
- **Node capability (#379 DP4/#516):**
  `NodeRepository::node_storage_class_rejection` (`nodes.rs:410`),
  fail-open on unreported nodes, called **before the transaction** so
  a rejection journals nothing (`vms.rs:536-545`; pinned zero-journal
  by `cmd/chvctl/tests/contract.rs:907-950`).
- **Core-managed posture (#378/#495):** the snapshot family rejects at
  accept via `ensure_volume_snapshot_family_supported` /
  `ensure_node_not_core_managed` (`lifecycle.rs:363-396`); clone checks
  its placement node (`lifecycle.rs:1296`). VM create on core-managed
  nodes works (Core executes it) — a *standalone volume* create would
  be a legacy-path stord side effect writing behind Core's back
  unless rejected.
- **Ownership (#386):** every `INSERT INTO volumes` production site
  stamps `owner_id` post-#386 (`vms.rs:671`, `imports.rs:221`,
  `templates.rs:422`; clone inherits the source's,
  `desired_state.rs:856-861`); `require_volume_owner`
  (`handlers/volumes.rs:429-454`) makes a NULL-owner volume
  **admin-only** — an unstamped create would lock its own creator out
  of mutating it.
- **Quota:** ~~`enforce_user_quota` (`vms.rs:1549+`) already counts
  `SUM(volumes.capacity_bytes)` — a standalone create is the first
  caller with `vm_count_delta = 0`.~~ **[Corrected 2026-10-06, #525:]**
  this census claim was WRONG — the enforcement SUM (and both quota
  meters) joined through `vm_desired_state` via `attached_vm_id`, so
  standalone volumes never accrued toward `used`; each create was
  capped individually but successive creates stacked past the
  aggregate limit. Found in the PR 2 review, fixed in #525/#526: one
  canonical `storage_usage_bytes` query (owner ∪ attach, no
  double-count) read by enforcement and both meters — see the #525
  CHANGELOG entry for the adopted counting rules.
- **Display name:** `is_valid_display_name` (`vms.rs:1511`,
  `^[A-Za-z0-9 ._-]{1,64}$` — the name is interpolated into volume
  names/paths, `vms.rs:329-335`).

### 2.6 What does NOT exist (scope truths)

- **No volume delete, anywhere**: no `DeleteVolume` RPC
  (`proto/controlplane/control-plane-node.proto:480-514`), no delete
  action in the BFF mutate arm (`bff_mutations.rs:426-475`:
  attach/detach/resize only). Volume rows are never deleted (the
  clone pre-check relies on it, `lifecycle.rs:1260-1262`). A create
  surface without a delete surface grows operator-managed residue —
  real LVs on LVM nodes, per #379's disclosed retention posture.
- **No capacity reporting/scheduling** (#379 §8 defers it): the
  accept-time check is class-only; free extents are invisible.
- **No seed path for standalone volumes** beyond what attach/VM-create
  already do; `seed_from` is refused on the LVM open path by design
  (`OPERATIONS.md:413-414`).

## 3. Goals & non-goals

**Goals**

- An operator (chvctl, UI, or raw API) can create a standalone data
  volume on a named node with a capacity and an optional storage
  class, and the volume is subsequently attachable via the existing
  attach path.
- Every accept-time discipline the platform has adopted since #274
  holds on the new surface: vocabulary 400, node-capability 400 with
  zero journaling, ownership stamped, quota enforced, display name
  validated, core-managed posture deliberate (DP7).
- The #372 DP8 freeze is honored verbatim — no renamed keys; the
  `volume create` harness row lands with the surface, not after.
- NULL/absent `storage_class` = local = today's semantics; the string
  is never materialized into specs for class-less volumes.

**Non-goals** (§8 restates with reopen triggers): volume delete;
capacity-aware scheduling; attach-at-create *execution* (DP4 defers
the key itself); seeding standalone volumes from images; iscsi/ceph
enablement; mixed-backend nodes; the `storage_pools` catalog's fate
(#379 Option B); the Option C typed contract crate.

## 4. Options — the dispatch carrier (the one genuinely open shape)

### Option A — new `CreateVolume` node RPC (recommended; #379 DP6's shape)

`message CreateVolumeRequest { RequestMeta meta; string node_id;
VolumeMutationSpec volume; }` beside `CreateVmRequest`
(`proto/controlplane-node.proto:196-200`); `volume_spec_json` =
`{"size_bytes": N, "backend_class": "lvm"?}`. The BFF journals
`CreateVolume`/Accepted BFF-direct (§2.3 precedent); the orchestrator
gains a `"CreateVolume"` arm that resolves the volume's class (the
claim query already resolves `volume_storage_class` for the attach
arm, `orchestrator.rs:882-899`, `:1833-1834`) and dispatches the RPC;
the agent handler opens via `open_volume_with_options` with the size
and DP5 locator shaping — `prepare_vm_resources` minus the attach.

*What changes:* one proto message + rpc, one node_client method, one
orchestrator arm, one agent handler, the BFF route. *What breaks:*
nothing — no producer exists before the BFF route, and the RPC is
additive beside the dead-but-live A10 `CreateVm`. *Costs:* a proto
change (regenerated, same-commit CP+agent deployment makes it cheap);
a second class-writing open site to pin (#379's A-site discipline).

### Option B — the fragment channel (`ApplyVolumeDesiredState`)

The orchestrator's `"CreateVolume"` arm dispatches
`apply_volume_desired_state` with the typed `VolumeSpec`
(`fragment.rs:42-55`); the agent's fragment handler is extended to
open-with-size when the spec carries capacity and no attachment.
*No proto change*, and the CP-side spec model already exists. But
three changes hide inside "reuse": the `"vm_id"` vs `attached_vm_id`
key drift must be fixed (§2.4 — today a CP-produced fragment **never**
triggers the agent's attach branch; the tests pin the raw key,
`agent_server.rs:4030-4046`), the standalone-open behavior is new
code on the legacy cache-write path, and the whole RPC fails closed
in core-managed mode (`agent_server.rs:540-544`) — which gives DP7's
posture for free but entangles it with the fragment cache semantics
(stale-generation checks, cache persistence). *Rejected as the v1
carrier:* it couples the new surface to the least-exercised agent
path and spends the fix budget on drift repair instead of the
feature; recorded as the consolidation candidate if the fragment
channel ever becomes the mainline volume carrier (§8).

### Option C — journal-only (row + desired state, no dispatch)

*Rejected on evidence* (§2.4): the minted volume can never be
attached — the attach open carries no size. This is
accepted-then-silently-failed, the #378 failure class, built in.

## 5. Decision points

### 5.1 Adoption record

**All decision points below were adopted as recommended by the
maintainer on 2026-10-06** (shape follows #379 §5.1 / #372 §5.1):
DP1 BFF-direct journaling; DP2 Option A, the new `CreateVolume` node
RPC, with the agent-side fail-closed core-managed posture; DP3 the
DP8-frozen contract fields plus the `volume_kind = 'data'` stamp and
the server-minted id; DP4 defer `attached_vm_id` (standalone-only v1,
loud 400 on the reserved key); DP5 the #379/#516 validation mirror;
DP6 ownership stamp + quota enforcement; DP7 core-managed reject at
accept; DP8 `volume_kind = 'data'`; DP9 the chvctl surface + harness
rows + cli-spec flip in one PR; DP10 wire the dead UI button and fix
the phantom Storage Driver column; DP11 no volume delete in scope,
follow-up issue filed.

**Decomposition (4 PRs, in landing order):**

1. **PR 1 — the dispatch carrier (dead-but-live, no producer):** the
   `CreateVolume` RPC on the node `LifecycleService` (additive proto),
   the `node_client::create_volume` method, the orchestrator's
   `"CreateVolume"` dispatch arm, and the agent's open-with-size
   handler (DP5 locator, DP7 fail-closed), with agent/CP/stord tests
   and this document. No BFF route, no chvctl, no UI — nothing
   reachable from outside (the #379 PR 1 precedent).
2. **PR 2 — the BFF route** (`POST /v1/volumes/create`): DP1's
   BFF-direct journaling + DP3/DP5/DP6/DP7 accept-time disciplines.
   **Must follow PR 1**: the orchestrator actively Fails unknown
   operation types (§2.3), so a journaled `CreateVolume` with no arm
   is the accepted-then-failed UX #378 was filed to kill.
3. **PR 3 — chvctl `volume create` + harness rows + cli-spec flip**
   (DP9; the 39-row/zero-pinned-broken terminal state preserved).
4. **PR 4 — optional UI**: wire the dead "Allocate Block" button and
   fix the phantom Storage Driver column (DP10).

**PR 1 = the PR carrying this document.** Census note: §2's line
references are pinned to `ce3e5b3d` (the commit the investigation
verified); PR 1 itself adds the `CreateVolume` RPC beside the
`proto:480-514` list, the orchestrator arm beside the §2.3 dispatch
table, and the agent handler beside the §2.4 open sites — every
"does not exist" claim in §1 gap 1's node-RPC clause is the fact this
PR changes, by design.

**DP1 — journaling layer: BFF-direct, mirroring `POST /v1/vms`.**
*Recommendation: yes.* The handler journals `volumes` +
`volume_desired_state` (`desired_status 'Pending'`,
`attached_vm_id NULL`) + `operations` (`'CreateVolume'`, `'Accepted'`,
idempotency key `create-volume-{volume_id}`) in one `BEGIN IMMEDIATE`
transaction, after the accept-time checks; response shape
`{accepted, task_id, volume_id, summary, next_refresh_path}` mirroring
`vms.rs:872-878`. *Alternative:* journal via a CP lifecycle
`CreateVolume` RPC (the `lifecycle.rs:663-729` shape) — rejected for
v1: the lifecycle create RPCs have no production caller (#516), and a
second journaling pipeline for the identical row shape is pure
duplication; the BFF-direct path is where #516 already wired the
capability check. *(PR 1 note: the CP-side gRPC lifecycle surface
gains a `create_volume` method only because the proto service is
shared — it answers `unimplemented` naming this decision, so no
second journaling pipeline is even reachable.)*

**DP2 — dispatch carrier: Option A, the new `CreateVolume` node RPC.**
*Recommendation: Option A* (§4) — it is #379 DP6's adopted
recommendation verbatim (`issue-379-storage-class-dispatch.md:508-516,
:570-571`), it puts the new open site beside the four the #379
plumbing already covers (A1–A4/A10 discipline), and the orchestrator
arm is five lines beside `"AttachVolume"`. The agent handler **must
fail closed in core-managed mode** exactly like its sibling legacy
volume ops (`agent_server.rs:506-518` posture) — see DP7. *Alternative:*
Option B (fragment channel) — deferred with its drift-repair
prerequisites recorded (§8).

**DP3 — the frozen contract (DP8 honored, plus this design's
additions).** Route `POST /v1/volumes/create`, operator tier, beside
`volumes/mutate|snapshot|clone` (`router.rs:299-316`). Fields exactly
per the #372 DP8 freeze list (`issue-372 design:413-446`):

- `name` + `display_name` alias, validated by `is_valid_display_name`;
- `node_id` — **required** (standalone volumes have no VM to place
  them; no first-enrolled-node default — silent placement of storage
  is worse than silent placement of a VM);
- `capacity_bytes` (i64, > 0, ≤ 64 TiB — `MAX_VOLUME_SIZE_GB`
  discipline, `vms.rs:16`; chvctl converts `--size 10G` via the
  existing `parse_size_bytes`, `vm.rs:258`);
- `storage_class` — optional, validated against
  `chv_hypervisor_api::resources` exactly like `vms.rs:497-515`
  (absent/blank → NULL → local; aliases rejected here);
- `seed_image_ref` — **deferred and rejected at accept if ever sent**
  (400 "not supported"; the freeze list already requires rejecting
  the non-local-class combination);
- `attached_vm_id` — **stays reserved, not accepted, in v1** (DP4).

Additive decisions this design contributes: `volume_kind` stamped
`'data'` (DP8 below); no client-supplied `volume_id` (server-minted
via `gen_short_id`, mirroring vm create — the clone path's
caller-supplied id exists for replay semantics this surface doesn't
need).

**DP4 — attach-at-create (`attached_vm_id`): defer; standalone-only
v1.** *Recommendation: v1 creates unattached volumes; the reserved
key is NOT accepted (a payload carrying it gets a 400 naming the
mutate-attach path, so the reservation is loud, not silently
dropped — the #372 `--vlan` lesson).* Rationale: attach already has
a complete, checked surface (the UI modal filters same-node VMs,
`[id]/+page.svelte:73-75`; the lifecycle attach RPC runs the class
check against the *volume row's* node, `lifecycle.rs:959-986`); wiring
attach into create means re-running VM-existence, same-node, and
running-state checks inside the create tx for zero new capability.
The freeze explicitly permits additive growth later
(`issue-372 design:433-436`) — when demanded, accept `attached_vm_id`
as an optional key and journal create + a second AttachVolume
operation, never a merged one. **When that attach lands (or any
standalone-attach path does), it must open the volume with the
carrier's relative `{volume_id}.img` locator — see the locator guard
under DP2; A1's vm-dir-nested locator and the A4 bare-id default
would mint a second default-size file and orphan the created one.**
*Alternative:* accept-and-journal-both
now — rejected: doubles the accept-time surface for a convenience.

**DP5 — storage-class validation: mirror #379/#516 exactly.**
*Recommendation:* (i) vocabulary via
`chv_hypervisor_api::resources::is_known_backend_class` — the one
shared list; (ii) node capability via
`node_storage_class_rejection` **before the transaction** — a
rejection writes no rows (pinned by the zero-journal assertion
pattern, `contract.rs:907-950`); (iii) fail-open on unreported nodes
(byte-exactly the lifecycle semantics, `vms.rs:530-535`); (iv) NULL
request class on an LVM-only node rejects (the check treats NULL as
local — same as vm create). No new vocabulary, no new check code —
both calls already exist and are shared.

**DP6 — ownership and quota: stamp and enforce.** *Recommendation:*
`owner_id = claims.sub` in the INSERT (the #386 invariant — an
unstamped volume is admin-only and locks out its creator,
`volumes.rs:429-454`); `enforce_user_quota(&mut tx, &claims.sub, 0, 0,
capacity_bytes, 0)` inside the transaction (the storage column of the
quota table is otherwise only reachable through VM creates).
Disclosure: this makes standalone creates the first direct
storage-quota consumer — ~~the quota UI's storage meter should count
them (it sums `volumes.capacity_bytes`, so it will, automatically).~~
**[Corrected 2026-10-06, #525:]** the meter did NOT count them (same
attach-join shape as enforcement — it also over-reported cpu/memory
via per-volume fan-out); both were fixed in #525/#526 with the
canonical `storage_usage_bytes` query.

**DP7 — core-managed nodes: reject at accept.** *Recommendation:*
mirror #378 — the BFF handler rejects a create targeting a
core-managed node with a 400 before journaling (the
`ensure_node_not_core_managed` decision lifted into
`node_storage_class_rejection`-style shared composition, or a direct
`get_authority_mode` read beside it). Justification: the provisioning
RPC is a legacy-path stord side effect; on a core-managed node the
single writer is Core, which cannot express a standalone volume
create (its `StorageAttachmentRef` has no standalone-volume concept,
#379 DP8). VM-embedded LVM disks still work there via Core's create —
the split posture is #378's recorded UX gap extended to a new
surface, disclosed, not worsened. *Alternative:* fail-open (accept,
fail at dispatch) — rejected: it is the exact #378 filing. *(PR 1
lands the dispatch-time half: the agent's handler refuses with
`unimplemented` and the orchestrator's #378 §7 fast-fail takes the
operation terminal without retry; PR 2 lands the accept-time 400.)*

**DP8 — `volume_kind`: stamp `'data'`.** *Recommendation:* set
`volume_kind = 'data'` on standalone creates. The column exists
(`0001:127`), is copied by clone (`desired_state.rs:856`), displayed
in the detail row (`volumes.rs:134`), and is NULL on every
VM-embedded boot disk today — stamping it gives operators the one
query that distinguishes data volumes from boot disks on the retention
boundary (there is no delete; §2.6). *Alternative:* leave NULL for
byte-exact row-shape parity with vm-created volumes — acceptable if
the maintainer prefers zero novel row shapes; the detail display
already COALESCEs.

**DP9 — chvctl surface and spec.** *Recommendation:*
`chvctl volume create <name> --node <node_id> --size <bytes|K|M|G>
[--storage-class <local|iscsi|ceph|lvm>]` — flags mirroring the DP9
`vm create` vocabulary (#372), `--size` via `parse_size_bytes`
(`vm.rs:258`), client-side class choices from the DP3 vocabulary with
the server as authority. The cli-spec's "not implemented" note
(`cli-spec.md:44`) flips in the same PR. The contract-harness rows
land in the same PR (DP11/test strategy): the #372 campaign's
terminal state — every row green, any future drift fails outright —
must not regress by shipping an unpinned new command.

**DP10 — UI: wire the dead button; fix the phantom column.**
*Recommendation:* the "Allocate Block" button (`+page.svelte:87-93`)
grows a create modal (name, node, size, storage class) posting to the
new route via `mutateWithRefresh()`; ~~the same PR fixes the "Storage
Driver" column to read the served field (add `storage_class` to the
list response — one SELECT column, `volumes.rs:38-63` — and read it
with a `local`-for-empty rendering, deleting the hard-coded
`'LOCAL_LVM'` fallback, `+page.svelte:78`).~~
**[Corrected 2026-10-06, #528:]** the column was REMOVED, not
repointed — the list API serves neither `backend` nor
`storage_class` (only the detail route carries the class), so the
column fabricated `'LOCAL_LVM'` for every row. The maintainer
ratified removal over the one-field BFF list-route addition and
declined a follow-up issue (2026-10-06); the per-volume class
remains visible on the volume detail page. *Alternative:* CLI-first,
UI later — acceptable; but the dead button is a standing lie on a
shipped page and the fix is small.

**DP11 — no volume delete in scope; file the follow-up.**
*Recommendation:* keep delete out (it is its own design — retention
posture, observed-state tombstones, agent-side LV/file removal,
`ON DELETE CASCADE` rows in VDS/VOS). File a tracking issue
("volume delete API") before the create PR merges, and disclose in
the create PR's description that created volumes are retained
indefinitely (real LVs on LVM nodes). *Alternative:* bundle a minimal
row-tombstone delete — rejected: high-risk (data-loss path, per
CONTRIBUTING's disclosure rule) and it doubles the review surface.

## 6. Test strategy

Existing pins to keep green: the full BFF suite (in particular
`tests/volume_snapshot_clone.rs` — the seeding shape every new row
reuses — and `tests/vm_create_storage_class.rs`, the accept-time 400
template); `cmd/chvctl/tests/contract.rs` (39 rows, zero
pinned-broken — the terminal #372 state); the store's clone/idempotency
suite; the agent's open-site tests (`reconcile.rs` test module) and
the stord create-on-open tests (`local.rs`, `lvm_real.rs` root-gated).

New per PR:

- **PR 1 (dispatch carrier, no producer):** agent-tier — the
  `CreateVolume` handler opens with size + class + DP5 locator
  against a mock stord, fails without size, fails closed in
  core-managed mode; CP-tier — the orchestrator `"CreateVolume"` arm
  resolves the class and dispatches (the `RecordingMutations`/
  mock-node pattern); proto regen check. *(As landed: the mock-stord
  log records the options map, the sizeless and unsafe-id refusals
  are pinned before any open, the core-managed refusal joins the
  `core_managed_legacy_effectors_fail_closed` set, the orchestrator
  tests pin the exact `volume_spec_json` bytes, the capacity-less
  retry-arm semantics, and the `Unimplemented` terminal fast-fail; a
  local-backend test pins create-on-open at the requested size; the
  CP↔agent contract pair pins the producer/consumer bytes on both
  sides. The capacity-less arm deliberately takes the ordinary retry
  curve (sibling-consistent) rather than a terminal class: a row with
  no capacity can never gain one, but the arm is unreachable by any
  correct producer — PR 2's DP3 accept-time validation guarantees
  `capacity_bytes > 0` before journaling — so it is defense-in-depth
  noise, not a live path.)*
- **PR 2 (BFF route):** happy path (row shapes: owner, kind, NULL
  class, `Pending` VDS with NULL `attached_vm_id`, `Accepted`
  operation); vocabulary 400; node-capability 400 with **zero
  journaling** across `volumes`/`volume_desired_state`/`operations`
  (the `contract.rs:907-950` table-loop assertion); core-managed 400
  (DP7); quota rejection; display-name rejection; `attached_vm_id`
  sent → 400 naming the attach path (DP4's loud reservation);
  unauthenticated/viewer → 401/403.
- **PR 3 (chvctl + harness + spec):** `volume_create_row` (green:
  route, fields, forwarded journal, `volume_id` in response);
  `volume_create_storage_class_rejection_row` (400 + zero journaling);
  `volume_create_requires_node_row` (400). cli-spec aligned in the
  same PR (the #372 lesson: the spec is a third voice that drifts).
- **PR 4 (optional UI):** modal posts and the list refreshes; ~~the
  Storage Driver column renders `storage_class`/`local`.~~
  **[Corrected 2026-10-06, #528:]** the column was removed per
  maintainer ratification (see the DP10 correction above).

Validation ladder: `cargo test -p chv-agent-core` /
`-p chv-controlplane-service` / `-p chv-webui-bff` / `-p chvctl`,
then `cargo test --workspace` + clippy/fmt; `cd ui && npm run build`
for PR 4.

## 7. Rollout & rollback

- **PR 1** adds a dead-but-live RPC + orchestrator arm — no producer,
  no behavior change (the #379 PR 1 precedent). Rollback is revert.
  *(As landed: the arm is additive beside the existing dispatch table;
  `CreateVolume` joins the placement-schedulability set; the CP-side
  gRPC lifecycle shim answers `unimplemented` per DP1.)*
- **PR 2** adds one operator-tier route — additive; nothing existing
  calls it. The accept-time checks reuse in-place compositions. The
  one durable-effect disclosure: created volumes are retained (no
  delete — DP11); on LVM nodes the provisioning dispatch creates real
  LVs at create time (not lazily at first attach) — operator-visible
  `lvs` output changes after the first create.
- **PR 3** is client + tests + docs; the harness must stay at zero
  pinned-broken (new rows land green).
- **PR 4** is UI-only.
- Sequencing constraint (§2.3): PR 2 **must not** land before PR 1 —
  a journaled `CreateVolume` operation with no dispatch arm is
  actively Failed by the orchestrator (`orchestrator.rs:1163`), which
  would ship the accepted-then-failed UX #378 was filed to kill.

## 8. Non-goals / scope boundaries (with reopen triggers)

- **Volume delete** — *reopen via the DP11 follow-up issue; a create
  surface without delete is disclosed, not hidden.*
- **Attach-at-create execution** — *reopen when an operator workflow
  demands it; the reserved key grows additively per the freeze
  (`issue-372 design:433-436`), and any standalone-attach path that
  lands must open the volume with the carrier's relative
  `{volume_id}.img` locator (the DP2 locator guard — not A1's
  vm-dir-nested path, not the A4 bare-id default).*
- **Seeding standalone volumes from images** (`seed_image_ref`) —
  *reopen when LVM gains a seed path or the demand is local-only;
  until then the key is rejected loudly.*
- **Fragment-channel consolidation (Option B)** — *reopen when the
  `"vm_id"`/`attached_vm_id` fragment-key drift is repaired and the
  fragment channel gains a production consumer; the drift repair is
  independently worth a small PR either way.*
- **Capacity-aware scheduling / free-extent reporting** — #379 §8's
  triggers, unchanged.
- **iscsi/ceph classes** — unqualified even at the stord layer
  (#379 §8); the vocabulary accepts them, the capability check will
  reject them on every real node today.
- **The typed shared contract crate (#372 Option C)** — *reopen when
  the surface stabilizes post-#513, per #372 §8.*

## 9. Residual risks

1. **A second class-writing open site.** The agent's `CreateVolume`
   handler joins A1–A4/A10; a future backend-class refactor must
   cover it. Mitigated by the PR 1 mock-stord pins (the #379 PR 1
   red/green discipline: reverting to a `"local"` literal must fail a
   test).
2. **Created-but-never-attached volumes accumulate** (no delete,
   DP11) — on LVM, real LVs. The `volume_kind = 'data'` stamp (DP8)
   is the operator's only sweep handle until delete exists.
3. **The reserved-key rejection could surprise API users** who read
   the freeze list as a promise of attach-at-create. The 400 message
   must name the mutate-attach path (DP4) — and the issue body should
   be updated if the maintainer adopts deferral (see scope
   corrections below). *(Done: the issue body records the deferral.)*
4. **Quota's storage column gains a direct writer** — a
   misconfigured `max_storage_bytes` now bites standalone creates
   where it previously only bit VM creates; the error message should
   name the volume (not just the quota row) for operability.
5. **Core-managed rejection (DP7) extends #378's UX gap** to a new
   surface: standalone volumes are impossible on core-managed nodes
   while VM-embedded LVM disks work. Disclosed, consistent with the
   single-writer boundary; Core modeling standalone volumes is the
   long-term fix (outside this design).
6. **The UI's phantom `backend` column (§2.2)** is fixed only if
   DP10 is taken; otherwise the create PR ships beside a list page
   that mislabels every volume's driver — flagged as scope-adjacent,
   not blocking.
7. **The freeze is enforced by the harness, not the doc** (#372
   residual 5, verbatim): if review changes a name here, the harness
   row and both design docs must move in the same PR.

---

## Appendix — suggested #513 issue-body corrections (scope)

1. **Open question 1 is settled by precedent:** journaling is
   BFF-direct (the production create path since #516's finding); the
   live question is only the dispatch carrier. The issue's "or
   dispatches directly (like the legacy A10 shape)" framing should be
   dropped — the A10 shape has no production caller.
2. **Add the provisioning constraint:** a journal-only create is not
   viable (attach carries no size, §2.4) — the issue's minimal shape
   should state that the dispatch MUST provision at create time.
3. **Open question 2 (`attached_vm_id`)**: this design recommends
   **defer** (standalone-only v1, loud 400 on the reserved key) — the
   issue should record the recommendation and the additive-growth
   path rather than leaving the question open.
4. **`node_id` required, not defaulted** — worth stating in the issue
   (the vm-create default-node behavior is not appropriate for
   storage placement).
5. **New dependencies the issue doesn't name:** ownership stamping
   (#386), quota enforcement (the storage column), core-managed
   rejection (#378 posture), and the no-delete disclosure (DP11
   follow-up issue).

*(All five corrections are reflected in the adopted issue body as of
2026-10-06.)*
