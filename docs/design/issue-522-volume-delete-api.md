# #522 design — volume delete: the standalone `DeleteVolume` surface (the #513 DP11 follow-up)

**Issue:** kubedoio/chv#522 — there is no way to delete a volume.
Volumes can be created (#513, landed as #523/#524/#526/#527/#528),
snapshotted, cloned, and attached — but never deleted. The issue is the
DP11 handoff from the adopted #513 design
(`docs/design/issue-513-volume-create-api.md:555-563`), which scoped
delete out of the create campaign precisely so it could get its own
carrier/journaling design. #513 stamped `volume_kind = 'data'` on
standalone creates (its DP8) as the retention boundary this issue
consumes.

**Status:** **ADOPTED 2026-10-06** — all 12 decision points below
were adopted as recommended by the maintainer, with the 4-PR mirror
decomposition recorded in §5.1. This is **PR 1 of the decomposition**,
and it carries both reclaim layers merged (the stord `DestroyVolume`
primitive of DP3 and the `DeleteVolume` node RPC of DP2, one atomic
dead-but-live unit — the §5.1 argument). The census below was taken at
main `14460ead` (verified `git rev-parse --short HEAD`); every §2 line
reference is pinned to that commit and was re-verified against the PR
1 branch point (same commit) before landing — the only drift found is
noted inline (§2.3's `lvm.rs:395-424` is `392-424` as landed, a
three-line offset with `lvremove` still at `:407`). As-landed
annotations where the implementation forced an adaptation are marked
**[As landed]** throughout and itemized in §5.1.

**Premise sharpening up front (the issue's scope list is directionally
right, with three corrections):**

1. **The "agent-side reclaim per backend" the issue names does not
   exist at any layer, and cannot be written in the agent alone.**
   stord's `StorageService` has no destroy/delete-volume RPC
   (`proto/node/chv-stord-api.proto:184-198`), the backend trait has no
   destroy method (`crates/chv-stord-backends/src/trait.rs:67-175`), and
   the local backend's `close` is a no-op on the file
   (`crates/chv-stord-backends/src/local.rs:382-388`). The agent cannot
   bypass stord safely: the local locator is RELATIVE and resolved
   against stord's own runtime dir (`local.rs:36-42`), and open volumes
   carry persisted stord sessions
   (`crates/chv-stord-core/src/handlers.rs:529-530`). **The reclaim
   primitive is a stord extension first; the node RPC is its consumer.**
   This materially widens PR 1 relative to #513's (§5.1 argues the
   decomposition).
2. **The platform has never reclaimed a backing store, by design, and
   #522 is the first surface that will.** Even VM delete stops at
   detach+close: the agent's `cleanup_vm_resources` detaches and closes
   stord sessions but never unlinks (`crates/chv-agent-core/src/
   reconcile.rs:2085-2186`), and the hypervisor layer explicitly
   disclaims it ("Disk images and the VM directory itself belong to the
   storage/authority layers", `crates/chv-agent-runtime-ch/src/process.
   rs:4147-4149`). This makes #522 a **high-risk, data-loss-path change**
   under CONTRIBUTING's disclosure rule — the design leans on
   accept-time guards (DP5–DP7) and idempotent, session-checked
   destruction (DP3) accordingly.
3. **The tombstone-vs-hard-delete question is not symmetric to the VM
   story, and the tree answers it.** Two facts make a hard row delete
   wrong: the orchestrator resolves the dispatch class from the
   `volumes` row at claim time (`orchestrator.rs:197-198,232-233`) —
   deleting the row at accept would strip the dispatch of its class —
   and the clone path's replay semantics assume "volume rows are never
   deleted" verbatim (`lifecycle.rs:1260-1262`). Recommendation: the
   `delete_vm` tombstone shape, with one quota predicate added (DP9).

---

## 1. Problem statement

Four gaps stack:

1. **No delete surface.** No BFF route (`crates/chv-webui-bff/src/
   router.rs:301-322` — create/mutate/snapshot/restore-snapshot/
   delete-snapshot/clone, no delete), no chvctl command
   (`cmd/chvctl/src/commands/volume.rs:8-50` — List/Create/Snapshot/
   Clone only), no node RPC (`proto/controlplane/control-plane-node.
   proto:496-517` — CreateVolume landed at `:502`, DeleteVm exists at
   `:506`, no DeleteVolume), no UI affordance (`ui/src/routes/volumes/
   [id]/+page.svelte:211-225` — Attach/Detach/Resize only). The CLI
   spec records the gap (`docs/specs/ops/chvctl-cli-spec.md:44-46`:
   "Volume show/delete are not implemented; volume delete is tracked in
   #522. Created volumes are retained indefinitely").
2. **No reclaim primitive.** §2.3 — nothing in the tree removes a
   backing file or LV; every delete-shaped surface stops at the stord
   session boundary.
3. **Unbounded operator-managed residue.** Every #513-created volume
   provisions a real artifact at create time (a sparse file on local, a
   real LV via `lvcreate` on LVM, `lvm.rs:146-160`); with no delete the
   artifacts accumulate forever — the disclosed #513 DP11 posture that
   this issue exists to end.
4. **The guards are unpicked.** Whether an attached volume, a boot
   disk, a volume with an in-flight operation, or a volume under an
   enabled backup schedule can be deleted — each is a decision this
   design must surface (DP5–DP7), mirroring the #378/#495 accept-time
   discipline #513 adopted.

The question for the maintainer is fivefold: (i) the journaling shape
(tombstone vs hard delete); (ii) the reclaim carrier (stord extension
vs agent-side); (iii) which volumes are deletable (kind, attachment,
references); (iv) how quota releases; (v) the surface cadence — each
with a recommendation below (§5).

## 2. Ground truth at `14460ead`

### 2.1 The #513 substrate (what create landed — delete's mirror)

The create campaign landed end-to-end and is the shape delete should
read as a sibling of:

- **Proto + carrier:** `CreateVolumeRequest`
  (`control-plane-node.proto:213-218`, meta + node_id + a
  `VolumeMutationSpec` whose `volume_spec_json` carries
  `{size_bytes, backend_class?}`), the `node_client::create_volume`
  method (`crates/chv-controlplane-service/src/node_client.rs:706-745`),
  the orchestrator's `"CreateVolume"` arm
  (`crates/chv-controlplane-service/src/orchestrator.rs:1002-1030` —
  capacity + class resolved in the claim query, refusal without a
  positive capacity), and the agent's open-with-size handler
  (`crates/chv-agent-core/src/agent_server.rs:1266-1385` — safe-id
  check, stale-generation check, size required, #379 DP5 class-dependent
  locator, `open_volume_with_options`, handle cached).
- **BFF route:** `POST /v1/volumes/create`
  (`crates/chv-webui-bff/src/handlers/volumes.rs:267-504`) — the
  accept-time discipline delete must mirror: reserved-key loud 400s
  (`:280-300`), display-name validation (`:311-315`), node capability
  via `node_storage_class_rejection` BEFORE the tx (`:385-392`),
  core-managed 400 (`:401-407`), `BEGIN IMMEDIATE` (`:413-417`), quota
  in-tx (`:423`), owner stamp (`:448`), `volume_kind 'data'` (`:442`),
  VDS `Pending`/NULL-attach (`:459-469`), `Accepted` operation with key
  `create-volume-{volume_id}` (`:475-488`), response
  `{accepted, task_id, volume_id, summary, next_refresh_path}`
  (`:497-503`).
- **chvctl + harness:** `volume create` (`cmd/chvctl/src/commands/
  volume.rs:13-33,80-133`) and its three contract rows
  (`cmd/chvctl/tests/contract.rs:1376,1493,1532`); the harness is at 43
  rows, zero pinned-broken.
- **UI:** the Allocate Block modal on the list page
  (`ui/src/routes/volumes/+page.svelte:99-104`).

### 2.2 The delete precedent — the VM-delete story (tombstone, no reclaim)

`POST /v1/vms/delete` (`crates/chv-webui-bff/src/handlers/vms.rs:
893-1050`) is the only journaled resource delete in production, and its
shape is the template:

- existence check → 404; `BEGIN IMMEDIATE`; `require_vm_owner` in-tx;
  **idempotent replay** against the recorded operation BEFORE any
  mutation (`:947-958`, key `delete-vm-{vm_id}` at `:939`, via
  `find_recorded_operation`, `crates/chv-webui-bff/src/handlers/
  operations.rs:43-58`);
- **tombstone, not row removal**: `UPDATE vm_desired_state SET
  desired_status = 'Deleting', desired_generation + 1` (`:975-985`) —
  the `vms`/`vm_desired_state` rows persist forever ("the M2.5
  authority-side retention keeps the VM rows after a delete",
  `:960-962`);
- one deliberate row cleanup where nothing else would ever do it (the
  nic rows, `:968-972`, the #356 lesson);
- one `Accepted` `DeleteVm` operation (`:988-1010`), response
  `{accepted, task_id, vm_id, summary, next_refresh_path}`
  (`:1030-1037`);
- dispatch: the orchestrator's `"DeleteVm"` arm
  (`orchestrator.rs:846-858`) → `node_client::delete_vm`
  (`node_client.rs:529`) → the agent's handler
  (`agent_server.rs:1602-1676`): stale-generation check,
  `cleanup_vm_resources` (detach + close stord sessions,
  `reconcile.rs:2085-2186`), `vm_runtime.delete_vm`, cache eviction.

**The load-bearing asymmetry:** the agent side of VM delete reclaims
NOTHING physical. `cleanup_vm_resources` detaches and closes
(`reconcile.rs:2159-2178`) but never unlinks; the hypervisor layer
removes only the api socket, pid file, and creation payload ("Disk
images and the VM directory itself belong to the storage/authority
layers", `process.rs:4147-4149`). The VM-delete "physical reclaim" the
#522 issue body assumes **does not exist** — volume delete must build
it (§2.3, DP3).

Downstream consumers already encode the tombstone posture: network
attached-VM counts filter `vds.desired_status != 'Deleting'`
(`crates/chv-webui-bff/src/handlers/networks.rs:50`), and the volume
list/detail read `COALESCE(vds.desired_status, vos.runtime_status,
'Unknown')` as status (`volumes.rs:43,129`) — a `'Deleting'` volume
renders as "Deleting" with zero UI work.

### 2.3 The missing primitive — no physical reclaim exists at ANY layer (the load-bearing finding)

- **stord's API has no destroy:** `StorageService` offers
  Open/Close/Attach/Detach/Resize/PrepareSnapshot/PrepareClone/
  RestoreSnapshot/DeleteSnapshot/SetDevicePolicy/GetVolumeHealth/
  ListVolumeSessions plus the disk-migration trio
  (`proto/node/chv-stord-api.proto:184-198`) — `DeleteSnapshot` is the
  closest, and it is name-scoped, not volume-scoped.
- **The backend trait has no destroy:** `StorageBackend`
  (`crates/chv-stord-backends/src/trait.rs:67-175`) — open, close,
  attach, detach, health, resize, snapshot/clone family, device policy,
  block I/O, `volume_size`, `create_receiving_volume`, dirty tracking.
  Nothing removes a volume.
- **The backends retain by construction:** local `close` only drops the
  dirty tracker (`local.rs:382-388`); LVM's only removal is
  `delete_snapshot`'s `lvremove` of the `-snap-` suffixed LV
  (`crates/chv-stord-backends/src/lvm.rs:395-424` **[as landed at
`14460ead`: `392-424`]**`, lvremove at `:407`).
- **The agent cannot do it alone:** the create carrier's local locator
  is the RELATIVE `{volume_id}.img`, resolved against stord's runtime
  dir inside the backend (`local.rs:36-42` — the agent never sees the
  absolute path); open volumes hold persisted session rows
  (`crates/chv-stord-core/src/handlers.rs:529-530`, SQLite-backed via
  `crates/chv-stord-core/src/store.rs:68`) that a raw `unlink` would
  strand; and the LVM VG name lives in stord's config
  (`agent_server.rs:1341-1347` reads it via `stord_backend.
  volume_group()` for locator shaping only).

So the delete carrier is **two layers**: a stord `DestroyVolume` RPC +
`StorageBackend::destroy` method (per-backend semantics), and the node
`DeleteVolume` RPC that closes the agent's session and calls it. §4
options; DP2/DP3 recommendations.

### 2.4 The locator discipline — and the still-open attach drift

#513's DP2 locator guard pinned the create carrier's locators: local =
the RELATIVE `{volume_id}.img`; LVM = the `/dev/mapper/{vg}-{vid}`
dm-path token (`agent_server.rs:1336-1348`). **Destroy must target
exactly these** (DP4) — anything else no-ops the reclaim and marks the
row `Deleting` anyway (silent leak, the exact failure class this design
exists to kill).

**Flagged, not folded (material to #522 scope):** the attach path's
default locator is STILL the bare `{volume_id}` for non-LVM classes
(`agent_server.rs:120-128`, `None => volume_id.to_string()`), and the
BFF's attach mutation sends a spec with no `locator` key
  (`node_client.rs:1489-1500`, `volume_attach_spec_json`). So a
standalone LOCAL volume that was ever attached has a SECOND stray file
at `runtime_dir/{volume_id}` (create-on-open at the 10 GiB default,
  `local.rs:15,281-355`) that the carrier-locator destroy will not
remove. This is the #513 §8 open guard, unfixed at `14460ead`. The
delete design does not fold the attach fix (it is the attach surface's
bug); it discloses it here, recommends the follow-up, and pins the
destroy locator to the CARRIER's (§8, residual 4).

### 2.5 The disciplines that must carry (already-built, one call each)

- **Ownership (#386/#481):** `require_volume_owner`
  (`volumes.rs:720-745`) gates every volume mutation surface (mutate
  `:523`, snapshot `:573`, restore `:612`, delete-snapshot, clone) —
  delete is the most consequential caller it will ever have; a NULL
  owner is admin-only by design.
- **Quota (#525/#526):** the canonical `storage_usage_bytes` query
  (owner ∪ attach, no double-count, `crates/chv-webui-bff/src/handlers/
  quotas.rs:308-328`) is read by enforcement (`enforce_user_quota`,
  `vms.rs:1564`, in-tx) and both usage meters. It has **no
  `desired_status` filter** — a tombstoned volume keeps accruing unless
  one predicate is added (DP9). Cross-user rule: one volume can accrue
  to two users (owner + attacher); delete releases both at once.
- **Core-managed posture (#378/#495):** `ensure_node_not_core_managed`
  (`crates/chv-controlplane-service/src/lifecycle.rs:384-398`) and the
  BFF-side direct read (`volumes.rs:401-407`, the create surface's DP7
  arm) — the agent's create handler fails closed with `unimplemented`
  (`agent_server.rs:1278-1285`) and the orchestrator fast-fails it
  terminal (pinned at `orchestrator.rs:2873-2938`). Delete mirrors all
  three halves (DP10).
- **Safe ids:** `chv_common::is_safe_id` at the node boundary
  (`agent_server.rs:1294-1300`) — a volume id becomes a path component
  of the destroy locator; the same traversal guard is load-bearing on a
  removal path, more than it was on create.
- **Idempotent replay (#406):** `find_recorded_operation` +
  `map_operation_insert_error` (`operations.rs:43-58,85+`) — the
  delete_vm pair, reusable verbatim for `delete-volume-{volume_id}`.
- **In-flight visibility:** operations rows are the task surface
  (`/v1/tasks`, `/v1/tasks/get`; the volume list/detail surface
  `last_task` from `operations` by `resource_kind = 'volume'`,
  `volumes.rs:53-57,137-141`); #502's terminal-failure cause
  (`error_code`/`error_message`) rides the same rows.

### 2.6 What does NOT exist (scope truths)

- **No delete anywhere** (§1 gap 1) — and specifically: no
  `DeleteVolume` on the node `LifecycleService`, no stord destroy, no
  BFF route, no chvctl verb, no UI affordance.
- **No volume-state machine to gate on.** `volume_desired_state.
  desired_status` is write-once `'Pending'` at create
  (`volumes.rs:459-469`) and by attach/detach patches
  (`lifecycle.rs:1017+`, `set_volume_attachment` with
  `desired_status: None`); nothing ever advances it. The display
  COALESCEs observed runtime status over it (`volumes.rs:43`). "Which
  states may be deleted" is therefore not a state-machine question but
  a guards question (DP5–DP8).
- **No FK'd dependents.** VDS/VOS cascade from `volumes`
  (`cmd/chv-controlplane/migrations/0001_initial.sql:134,151`);
  `backup_jobs.volume_id`/`backup_schedules.volume_id` are plain TEXT
  with no FK (`0024_backups.sql:13,29`) — but the backup worker
  genuinely claims schedules into jobs carrying the volume_id
  (`crates/chv-controlplane-service/src/backup_worker.rs:155-175`), so
  a schedule on a deleted volume keeps minting jobs. Clone lineage
  (`volumes.parent_volume_id`, `snapshot_chain`, `0025_volume_snapshot_
  clone.sql`) is journaled but read by no production code; VDS
  `clone_source_volume_id` is a dangling text reference at worst.
- **No force-detach-on-delete, no GC, no retention policy** — and the
  clone-replay assumption "volume rows are never deleted"
  (`lifecycle.rs:1260-1262`) is a line of code this design must either
  preserve (tombstone) or deliberately break (hard delete).

## 3. Goals & non-goals

**Goals**

- An operator (chvctl, UI, or raw API) can delete a standalone data
  volume: the CP tombstones the intent, the dispatch reclaims the node
  artifact (file or LV), and the volume disappears from operator
  surfaces while its rows persist as the delete tombstone.
- Every accept-time discipline holds: attached → loud 400 naming
  detach-first; boot disks → loud 400 naming the VM lifecycle; in-flight
  ops → 409; enabled backup schedule → 400; ownership via
  `require_volume_owner`; core-managed → 400 at accept and
  `unimplemented` at dispatch; quota releases on tombstone.
- The destroy is idempotent (absent artifact = success) and
  session-checked (an open stord session refuses), so crash-redrive and
  operator retry are both safe.
- The #513 cadence is mirrored: carrier (dead-but-live) → route →
  chvctl + harness + cli-spec → UI, with the harness staying at zero
  pinned-broken.

**Non-goals** (§8 restates with reopen triggers): force-delete past the
attached guard; deleting VM-embedded/boot-disk volumes (NULL
`volume_kind`); the attach-path locator fix (flagged, §2.4); snapshot
tree reclamation; iscsi/ceph destroy; `volume show`; retention/GC
policies; reclaiming pre-#513 embedded-disk residue.

## 4. Options — the reclaim carrier (the one genuinely open shape)

### Option A — stord `DestroyVolume` RPC + `StorageBackend::destroy` (recommended)

One stord proto rpc (`DestroyVolume(DestroyVolumeRequest) returns
(Result)` beside `DeleteSnapshot`, `chv-stord-api.proto:193`), one
trait method (`destroy(&self, volume_id, handle?)`), and four backend
implementations: local removes `runtime_dir/{locator}` if present
(absent = Ok, idempotent); LVM `lvremove -y {vg}/{vid}` if the path
exists (absent = Ok); iscsi/ceph return `InvalidArgument` refusing
loudly (they are unqualified even at open, #379 §8). The stord handler
refuses while an open session exists for the volume (the agent closes
its cached handle first; stord defends for every other caller). The
agent's `DeleteVolume` node-RPC handler then: safe-id check →
stale-generation check → close cached handle if any → destroy with the
DP4 locator → evict the cache entry and persist.

*What changes:* one stord rpc + trait method + 4 impls + handler, one
agent daemon-client method (`daemon_clients.rs`, beside `close_volume`
at `:221`), one node proto rpc, one `node_client` method, one
orchestrator arm, one agent handler. *What breaks:* nothing — no
producer exists before the BFF route, and the stord rpc is additive.
*Costs:* a second proto to regenerate (stord + control-plane — the
#513 note that same-commit CP+agent deployment makes proto changes
cheap extends to stord, which ships in the same node package); the
first data-destroying primitive in the tree (high-risk disclosure).

### Option B — agent-side direct removal (no stord change)

The agent unlinks the file / runs `lvremove` itself, using
`stord_backend.volume_group()` and a runtime-dir assumption for path
resolution. *Rejected on evidence:* the agent does not know the local
backend's runtime dir (the locator is relative by design, §2.3); it
would duplicate the LVM vg/`lvremove` command shaping stord already
owns (`lvm.rs:395-424`); and it would strand persisted stord session
rows for any volume stord still considers open. It is also a second
authority over storage layout — the exact split the ADR set out to
avoid.

### Option C — row-tombstone only (no reclaim, ever)

*Rejected on evidence:* it is the #513 DP11 status quo wearing a new
route — the operator gains a `Deleting` badge and keeps the LV. On LVM
nodes the residue is real extents; the issue exists to reclaim them.

## 5. Decision points

### 5.1 Adoption record

**All 12 decision points below were adopted as recommended by the
maintainer on 2026-10-06** (shape follows #513 §5.1): DP1 BFF-direct
tombstone; DP2 the new `DeleteVolume` node RPC; DP3 the stord
`DestroyVolume` primitive with per-backend semantics; DP4 the
create-carrier locator discipline; DP5 attached → loud 400, no force;
DP6 `volume_kind` 'data'-only; DP7 in-flight 409 + backup-schedule
400; DP8 no state-machine gating beyond the guards, `'Deleting'` is
terminal for volume verbs; DP9 quota release via the
`storage_usage_bytes` predicate; DP10 core-managed reject at accept +
fail-closed dispatch; DP11 chvctl + harness + cli-spec in one PR; DP12
detail-page Delete button with confirm. The 4-PR mirror decomposition
was adopted as proposed (below). **PR 1 carries both reclaim layers
merged** (stord primitive + node carrier, the one-atomic-unit argument
below), ships this document, and lands dead-but-live — nothing
dispatches it until PR 2's route journals the first `DeleteVolume`
operation.

**Proposed decomposition (4 PRs, in landing order — the #513 mirror,
argued):**

1. **PR 1 — the reclaim primitive + dispatch carrier (dead-but-live, no
   producer):** the stord `DestroyVolume` rpc + `StorageBackend::
   destroy` (local/LVM/iscsi/ceph) + stord handler (session refusal),
   the agent daemon-client method, the `DeleteVolume` node RPC, the
   `node_client::delete_volume` method, the orchestrator's
   `"DeleteVolume"` arm, and the agent's close→destroy→evict handler
   (DP4 locator, DP10 fail-closed), with stord/agent/CP tests and this
   document. No BFF route, no chvctl, no UI — nothing reachable from
   outside (the #513 PR 1 precedent). *Why one PR and not two:* the
   stord rpc alone is dead code with no consumer, and the node RPC
   without stord is unlandable (its handler would call a method that
   does not exist) — they are one atomic dead-but-live unit. *The
   5-PR alternative* (stord extension first, node carrier second) is
   recorded for the maintainer if review prefers smaller diffs; the
   split point is exactly between the stord proto/trait/impls and the
   node proto/arm/handler.
2. **PR 2 — the BFF route** (`POST /v1/volumes/delete`): DP1's
   tombstone transaction + DP5/DP6/DP7/DP9/DP10 accept-time guards.
   **Must follow PR 1**: the orchestrator actively Fails unknown
   operation types (`orchestrator.rs:1203`), so a journaled
   `DeleteVolume` with no arm is the accepted-then-failed UX #378 was
   filed to kill.
3. **PR 3 — chvctl `volume delete` + harness rows + cli-spec flip**
   (DP11; the 43-row/zero-pinned-broken terminal state preserved).
4. **PR 4 — UI** (DP12): the detail-page Delete button + confirm.

**PR 1 = the PR carrying this document.** Census note: §2's line
references are pinned to `14460ead`; PR 1 adds the stord rpc beside
the `chv-stord-api.proto:193` list, the trait method beside
`trait.rs:124-130`, the node rpc beside `control-plane-node.proto:502`,
the orchestrator arm beside the §2.1 dispatch table, and the agent
handler beside `agent_server.rs:1266` — every "does not exist" claim in
§1 gap 1–2 is the fact this PR changes, by design.

**As landed (PR 1), two adaptations the draft's letter required —
flagged, not silent:**

1. **`StorageBackend::destroy` takes the locator, not a handle.** The
   draft's DP3 sketched `destroy(&self, volume_id, handle?)`; as
   landed it is `destroy(&self, volume_id, locator: &BackendLocator)`.
   The draft's own §2.3 finding forces it: the local locator is
   RELATIVE and resolved against stord's runtime dir inside the
   backend, and a volume being destroyed has no handle (it was never
   opened, or was closed first — the session refusal guarantees the
   latter). The locator is the create carrier's (DP4), threaded from
   the agent through stord's `DestroyVolume` request exactly as an
   open would thread it, and it runs the same class-conditional
   allowlist checks at the stord boundary (`check_allowlist`,
   `check_path_allowlist` for local classes, `check_device_allowlist`
   for lvm/block) that `open_volume` runs — a removal path deserves
   the write path's hardening, if not more.
2. **`DeleteVolumeRequest` carries a `backend_class` field.** The
   draft's DP2 sketched the request as `{meta, node_id, volume_id}`
   with the class "resolv[ing] in the claim query like the attach
   arm's" — but the attach arm resolves the class in the claim query
   *and then rides it on the RPC* (through `VolumeMutationSpec`'s
   `volume_spec_json`); a delete request with no class field would
   leave the agent unable to shape the DP4 locator (LVM's dm-path
   token vs local's `{volume_id}.img`). As landed the request is
   `{meta, node_id, volume_id, backend_class}` with the NULL-class
   discipline preserved: a NULL `storage_class` on the volume row
   emits the EMPTY string, never a materialized `"local"` (the #511
   wire-key seam), and the agent's empty→local default is the create
   handler's own B5 seam.

Two smaller as-landed notes: the LVM `destroy` resolves its LV from
the backend's own VG plus the sanitized volume id (`{vg}/{vid}` in the
`lvremove` argument, mirroring `delete_snapshot`'s command shaping)
rather than parsing the dm-path locator string — the locator is
allowlist input at the stord boundary, not a command argument; and the
local `destroy` maps the `NotFound` errno to `Ok(())` (a missing
parent directory is the same errno as a missing file, and both are the
already-gone case the idempotency contract calls success).

---

**DP1 — journaling layer and shape: BFF-direct tombstone, mirroring
`POST /v1/vms/delete`.** *Recommendation:* the handler journals, in one
`BEGIN IMMEDIATE` transaction after the accept-time checks:
`UPDATE volume_desired_state SET desired_status = 'Deleting',
desired_generation = desired_generation + 1, updated_by = ?` (the
`vms.rs:975-985` statement, volume-shaped) + an `operations` INSERT
(`'DeleteVolume'`, `'Accepted'`, resource_kind `'volume'`,
idempotency key `delete-volume-{volume_id}`), with the #406 replay
pre-check (`find_recorded_operation`) returning the recorded outcome
on retry. Response `{accepted, task_id, volume_id, summary,
next_refresh_path}` mirroring `vms.rs:1030-1037`. The `volumes` row is
NOT deleted — three tree facts force it: (i) the orchestrator's claim
query resolves the dispatch class from the `volumes` row
(`orchestrator.rs:197-198,232-233`) — a hard delete at accept strips
the dispatch of its class; (ii) clone replay assumes "volume rows are
never deleted" (`lifecycle.rs:1260-1262`) — a deleted source turns a
legitimate replay into a NotFound; (iii) the task surfaces join
`operations.resource_id` against living rows for display
(`volumes.rs:53-57`). *Alternative:* hard-delete rows after the agent
acks (a reaper) — rejected for v1: it re-opens (i)–(iii) for zero
operator value (the tombstone already leaves the list surfaces via
DP9's predicate if desired) and adds a crash window between ack and
reap.

**DP2 — dispatch carrier: the new `DeleteVolume` node RPC (Option A's
node half).** *Recommendation:* `rpc DeleteVolume(DeleteVolumeRequest)
returns (AckResponse)` beside `CreateVolume`
(`control-plane-node.proto:502`), request shape `{meta, node_id,
volume_id}` (no mutation spec needed — the volume's class resolves in
the claim query like the attach arm's, `orchestrator.rs:882-899`; no
size rides a delete); `node_client::delete_volume` mirroring
`create_volume` (`node_client.rs:706-745`); an orchestrator
`"DeleteVolume"` arm beside the CreateVolume arm (`:1002-1030`) —
simpler than create's: no capacity refusal, class only. The agent
handler (PR 1) **must fail closed in core-managed mode** with
`unimplemented` exactly like `create_volume` (`agent_server.rs:
1278-1285`) so the orchestrator's #378 §7 fast-fail takes the refused
operation terminal without retry. *Alternative:* dispatch through the
fragment channel — rejected for the same reasons #513 rejected it (the
least-exercised agent path, `vm_id`-key drift, wholesale fail-closed
coupling).

**DP3 — the stord reclaim primitive: `DestroyVolume` rpc +
`StorageBackend::destroy` (Option A's stord half).** *Recommendation:*
land Option A (§4). Per-backend semantics: **local** — remove the
resolved locator path if present, absent = `Ok(())` (idempotent;
crash-redrive and stale caches must not turn a successful delete into
a failure); **LVM** — if `/dev/{vg}/{vid}` exists, `lvremove -y`
(shaping mirroring `delete_snapshot`, `lvm.rs:395-424`), absent =
`Ok(())`; **iscsi/ceph** — `InvalidArgument` refusing loudly (the
issue's own clause; they are unqualified at open too). The stord
handler refuses while a session exists for the volume (session lookup
beside `close_volume`'s, `handlers.rs:541-559`) — the agent is the
cooperative caller (it closes first), stord is the defense for every
other caller. *Alternative:* Option B (agent-side removal) — rejected
(§4: path resolution, command duplication, stranded sessions).

**DP4 — the locator guard: destroy targets the CREATE carrier's
locator.** *Recommendation:* the agent shapes the destroy locator
byte-identically to its create handler — local: the RELATIVE
`{volume_id}.img`; LVM: `lvm_locator(vg, volume_id)`
(`agent_server.rs:1336-1348`) — and the safe-id check runs first
(`:1294-1300`; a traversal on a REMOVAL path is worse than on a
create). **Disclosure (§2.4):** a standalone local volume that was
ever attached may carry a stray `runtime_dir/{volume_id}` file from
the attach path's bare-id default (`agent_server.rs:120-128`) — the
destroy deliberately does NOT chase it (folding the attach fix would
couple this design to another surface's bug); the follow-up issue for
the attach locator is recommended in the appendix. *Alternative:*
also sweep the bare-id path — rejected for v1: it hard-codes the
attach bug's artifact into the reclaim contract.

**DP5 — the attached guard: reject at accept, no force flag.**
*Recommendation:* a volume whose VDS `attached_vm_id` is non-NULL gets
a loud 400 naming the detach-first path (mirroring #513 DP4's
mutate-attach stance: `detach it via POST /v1/volumes/mutate with
action 'detach' first`) — checked inside the `BEGIN IMMEDIATE` tx so a
~~concurrent attach cannot slip in~~ [Corrected 2026-10-06, #535:] the
BFF tx only serializes the delete's own guard-read/tombstone-write
window — an attach whose CP-side journal lands AFTER the delete
commits could still overwrite the tombstone, because the CP mints
wall-clock-millisecond generations and the UPSERTs' generation guard
never blocks a late mutation against the tombstone's small integer.
The route-PR review folded the real backstop: the three mutation-verb
patch UPSERTs (attach/detach, resize, snapshot) now refuse to journal
over `desired_status = 'Deleting'` at the SQL level, mapping the block
to a loud Conflict (409 through `map_ack`, the #384 precedent). One tree-suggested refinement: an
attachment to a VM whose `vm_desired_state.desired_status =
'Deleting'` does NOT count as attached (the VM-delete story tombstones
the VM rows and never clears volume VDS `attached_vm_id`, §2.2 —
without this predicate, a volume whose VM was deleted could never be
deleted; the predicate is `networks.rs:50`'s exact filter). **No
`force` flag in v1** — force-on-delete is data-loss-plus-live-disk in
one key; the VM delete RPC carries one (`DeleteVmRequest.force`) but
the BFF never sends it (`orchestrator.rs:846-858` passes `false`
hard-coded). *Alternative:* accept `force` and journal a
detach+delete pair — rejected: doubles the accept surface on the most
dangerous verb in the design.

**DP6 — the kind gate: only `volume_kind = 'data'` is deletable.**
*Recommendation:* a volume whose `volume_kind` is not `'data'` (NULL
on every VM-embedded disk, §2.6) gets a 400 naming the VM lifecycle
("boot disks and VM-embedded volumes are managed by their VM's
lifecycle; delete the VM or detach and reclassify"). Two arguments:
(i) the issue's own scope clause (boot disks die with their VM — and
today they do NOT die with their VM, they are retained; that is a
VM-delete design question, not a volume-verb question); (ii)
**locator correctness** — embedded local disks live at the vm-dir-
nested `{runtime_dir}/{vm_id}/{volume_id}.img` (the A1 shape), which
the carrier locator would MISS: a delete of an embedded volume would
tombstone the row and reclaim nothing. The kind gate is also the
locator gate. Clones inherit the source's kind
(`crates/chv-controlplane-store/src/desired_state.rs:856`), so the
#513-era standalone lineage is deletable end-to-end. *Disclosed
consequence:* NO pre-#513 volume is deletable (all are embedded
lineage, NULL kind) — the legacy-residue sweep remains open (§8,
residual 6). *Alternative:* allow NULL-kind volumes whose attachment
is clear — rejected: silently reclaims nothing on local (wrong
locator) and invites embedded-disk data loss.

**DP7 — reference guards at accept: in-flight operations (409) and
enabled backup schedules (400).** *Recommendation:* inside the tx,
(i) reject with `BffError::Conflict` (409 — a transient condition, not
a validation error; `Conflict` exists, `crates/chv-webui-bff/src/
error.rs:14`) when an incomplete operation exists for the volume
(`operations WHERE resource_kind = 'volume' AND resource_id = ? AND
status IN ('Accepted','Running')`) — the issue's "no in-flight
operations" clause, and it covers the Pending-with-in-flight-create
case (a volume whose CreateVolume never dispatched); (ii) reject with
a 400 when an enabled `backup_schedules` row names the volume
(`0024_backups.sql:29`; the worker will keep claiming it into jobs
otherwise, `backup_worker.rs:155-175`) — the message names the
schedule id and the disable path. Both are one EXISTS each, before
any journaling, so a rejection writes no rows (the zero-journal
discipline, `contract.rs:946-991`). *Alternative:* skip the schedule
guard and let jobs fail downstream — rejected: it is the
accepted-then-failed UX on a surface that deletes data.

**DP8 — the state machine: no gating beyond the guards; `'Deleting'`
is terminal for volume verbs.** *Recommendation:* any volume passing
DP5/DP6/DP7 is deletable — there is no state machine to gate on
(§2.6: `desired_status` is write-once `'Pending'` and never advances),
and the destroy's idempotency (DP3) makes a partially-created volume
safe to delete: a Pending volume whose create dispatch never landed
reclaims nothing (absent artifact = Ok); one whose open landed but
whose ack was lost still reclaims. Symmetrically, once a volume is
`'Deleting'`, the sibling verbs reject it: mutate/snapshot/restore/
delete-snapshot/clone add a one-predicate `desired_status =
'Deleting'` rejection (the `networks.rs:50` discipline, applied to the
volume surfaces in PR 2) — without it, an attach raced against a
delete would create-on-open a fresh default-size file behind the
tombstone (the §2.4 failure class, manufactured by this design's own
window). *Alternative:* block `'Pending'` volumes until their create
op resolves — rejected: DP7 already blocks the in-flight case, and a
terminally-failed create is exactly the volume operators most need to
delete.

**DP9 — quota: release on tombstone, one predicate.** *Recommendation:*
add `AND vds.desired_status != 'Deleting'` (LEFT-JOIN-safe: a volume
with no VDS row still accrues via ownership, matching today's
semantics for orphan rows) to `storage_usage_bytes`
(`quotas.rs:308-328`). Enforcement (`enforce_user_quota`, in-tx) and
both usage meters read the same helper, so one predicate releases the
owner's AND the attacher's accrual simultaneously (the #526 cross-user
rule) with no second query. This is the one place the volume tombstone
differs from the VM tombstone — VM counts deliberately keep accruing
today (`vm_desired_state` has no such filter in `compute_usage_payload`)
— and the volume side should NOT copy that: storage quota is the
scarce resource #525/#526 existed to make honest. *Alternative:*
release only on a post-ack reaper — rejected with DP1's reaper.

**DP10 — core-managed nodes: reject at accept, fail closed at
dispatch.** *Recommendation:* mirror #513 DP7 on all three halves: the
BFF handler rejects a delete targeting a core-managed node's volume
with a 400 before journaling (the `get_authority_mode` read beside the
create surface's arm, `volumes.rs:401-407`); the agent's handler
refuses with `unimplemented` (`delete_volume is unsupported in
core-managed mode`); the orchestrator's Unimplemented fast-fail takes
the operation terminal. Justification: the destroy is a legacy-path
stord side effect; on a core-managed node the single writer is Core.
Note the asymmetry with create: a core-managed node's volumes are
un-creatable via this surface anyway, so this guard mostly covers
authority-mode transitions (a volume created on a legacy node whose
node later went core-managed) — the delete is still refused, the rows
still tombstone-able only after the node returns to legacy authority.
*Alternative:* fail-open — rejected: it is the #378 filing, on a
data-loss path.

**DP11 — chvctl surface and spec.** *Recommendation:* `chvctl volume
delete <volume_id>` — positional id, mirroring `vm delete <vm_id>` /
`network delete <network_id>` / `image delete <image_id>`; body
`{"volume_id": ...}` to `POST /v1/volumes/delete`; output mirrors
`vm delete`'s task-carrying response. No `--force` flag (DP5), no
`--kind` override (DP6). The cli-spec's not-implemented note flips in
the same PR (`cli-spec.md:44-46` — keep the `volume show` clause). The
contract-harness rows land in the same PR: the #372/#513 terminal
state — every row green, zero pinned-broken — must not regress by
shipping an unpinned destructive command.

**DP12 — UI: detail-page Delete button with confirm; no list-row
delete.** *Recommendation:* a `danger`-variant Delete button on the
volume detail page beside Attach/Detach/Resize
(`ui/src/routes/volumes/[id]/+page.svelte:211-225`), disabled when
`attached_vm_id` is set (the DP5 stance, visible), using the page's
existing in-place `confirmingAction` Confirm/Cancel pattern (not
`window.confirm` — the page already has the better primitive) with
copy naming the volume, its size, and irreversibility ("the backing
store on the node is destroyed; this cannot be undone"). Mutation via
`mutateWithRefresh()` (the AGENTS.md rule). **No delete affordance on
the list page** in v1 — a destructive verb on a table row is one
mis-click from data loss; the images page's list-row delete
(`ui/src/routes/images/+page.svelte:83-89`) is the counter-precedent
explicitly not followed. *Alternative:* list-row delete with confirm —
acceptable if the maintainer prefers parity with images; argued
against because volume data is user-authored, image artifacts are
re-importable.

## 6. Test strategy

Existing pins to keep green: the full BFF suite (in particular
`tests/volume_create_route.rs` — the accept-time template this design
mirrors — `tests/volume_creation_ownership.rs`,
`tests/volume_quota_standalone.rs`, `tests/volume_snapshot_clone.rs`);
`cmd/chvctl/tests/contract.rs` (43 rows, zero pinned-broken); the
agent's create/delete-site tests (`agent_server.rs` test module,
`create_volume_*` at `:4495+`, `delete_vm_cleans_up_storage_and_
network_resources` at `:3954`); the stord backend suites (`local.rs`,
`lvm_real.rs` root-gated).

New per PR:

- **PR 1 (primitive + carrier, no producer):** stord-tier — local
  destroy removes the file and is idempotent on the absent case; LVM
  destroy shapes `lvremove -y` and is idempotent (command-shaping mock
  or root-gated); iscsi/ceph refuse with `InvalidArgument`; open
  session refuses. Agent-tier — the `DeleteVolume` handler closes the
  cached handle before destroying (mock-stord log ordering pin),
  destroys with the CARRIER locator (`{volume_id}.img` /
  dm-token — reverting to the bare id must fail a test, the #379 PR 1
  red/green discipline), evicts the cache entry, fails closed in
  core-managed mode (joins the `core_managed_legacy_effectors_fail_
  closed` set), rejects unsafe ids. CP-tier — the orchestrator
  `"DeleteVolume"` arm resolves the class and dispatches
  (RecordingMutations/mock-node pattern); the `Unimplemented`
  terminal fast-fail; proto regen check (both protos). *(As landed: every named row shipped — local
  remove+idempotent and the DP4 stray-file pin (the bare-id file
  survives), LVM absent-case idempotency + unsafe-id + wrong-class
  (the live `lvremove` leg is root-gated in `lvm_real.rs`,
  `lvm_real_destroy_removes_the_lv_and_is_idempotent`), iscsi/ceph
  loud refusals, the stord handler's session refusal + allowlist
  refusal + idempotent happy path, the agent's close→destroy→evict
  ordering pin (event log), the cached-handle and uncached variants,
  the unsafe-id rejection, the core-managed set membership, and the
  orchestrator's class threading + `Unimplemented` terminal fast-fail;
  the CP-side gRPC lifecycle shim answers `unimplemented` naming
  DP1's adopted BFF-direct journaling, the #513 convention.)*
- **PR 2 (BFF route):** happy path (row shapes: VDS `'Deleting'`,
  generation bumped, `Accepted` `DeleteVolume` operation, key
  `delete-volume-{id}`); attached → 400 naming the mutate-detach path
  with zero journaling across `volumes`/`volume_desired_state`/
  `operations` (the `contract.rs:946-991` table-loop assertion);
  attached-to-deleting-VM → allowed (the DP5 refinement, pinned);
  boot-disk/NULL-kind → 400; in-flight op → 409; enabled backup
  schedule → 400; core-managed → 400; idempotent retry → 200 replaying
  the recorded outcome; not-found → 404; viewer → 403; quota: a
  `'Deleting'` volume stops accruing in `storage_usage_bytes` (extend
  `volume_quota_standalone.rs`, owner AND attacher sides); sibling
  verbs reject a `'Deleting'` volume (DP8's one-predicate guard).
- **PR 3 (chvctl + harness + spec):** `volume_delete_row` (green:
  route, field, journaled tombstone, task response);
  `volume_delete_attached_rejection_row` (400 + zero journaling);
  `volume_delete_in_flight_rejection_row` (409). cli-spec aligned in
  the same PR (the #372 lesson: the spec is a third voice that
  drifts).
- **PR 4 (UI):** the delete button posts and the detail/list refresh;
  the confirm gate; the disabled-when-attached state.

Validation ladder: `cargo test -p chv-stord-backends -p
chv-stord-core` / `-p chv-agent-core` / `-p chv-controlplane-service`
/ `-p chv-webui-bff` / `-p chvctl`, then `cargo test --workspace` +
clippy/fmt; `cd ui && npm run build` for PR 4.

## 7. Rollout & rollback

- **PR 1** adds dead-but-live rpcs (stord + node) and an orchestrator
  arm — no producer, no behavior change (the #513 PR 1 precedent).
  Rollback is revert. Deployment note: the stord proto change means
  the agent and stord must ship in the same node package version (they
  already do — one repo, one node bundle); the CP ships in the same
  commit as usual.
- **PR 2** adds one operator-tier route — additive; nothing existing
  calls it. The guards reuse in-place compositions. **Durable-effect
  disclosure (the CONTRIBUTING high-risk rule):** after the first
  delete, operator-visible `lvs`/directory output on the node SHRINKS —
  real LVs and files are destroyed, irreversibly, by design. The PR
  description must carry this and link the test evidence.
- **PR 3** is client + tests + docs; the harness must stay at zero
  pinned-broken (new rows land green).
- **PR 4** is UI-only.
- Sequencing constraint: PR 2 **must not** land before PR 1 — a
  journaled `DeleteVolume` operation with no dispatch arm is actively
  Failed by the orchestrator (`orchestrator.rs:1203`), which would
  ship the accepted-then-failed UX #378 was filed to kill, on a
  destructive verb.

## 8. Non-goals / scope boundaries (with reopen triggers)

- **Force-delete past the attached guard** — *reopen when an operator
  workflow genuinely demands tear-down-with-VM; until then detach
  first (the guard's 400 names the path).*
- **Deleting VM-embedded/boot-disk volumes (NULL `volume_kind`)** —
  *reopen via a VM-delete-design issue: embedded disks dying with
  their VM is a VM-lifecycle question (and their reclamation needs the
  A1 nested locator, a different carrier shape). The legacy-residue
  sweep (every pre-#513 volume) stays open — flagged, not solved here.*
- **The attach-path locator fix (§2.4)** — *the stray
  `runtime_dir/{volume_id}` file from a bare-id attach of a standalone
  local volume is the attach surface's bug; file the follow-up, fix it
  there, do not chase it in destroy (DP4).*
- **Snapshot-tree reclamation** — destroying a volume with existing
  stord-side snapshots: LVM `lvremove` semantics surface as a loud
  dispatch failure (safe direction); enumerating and cascading
  snapshots is *reopen when a snapshot-catalog surface exists (the VDS
  `snapshot_chain` column is journaled but unread, §2.6).*
- **iscsi/ceph destroy** — unqualified even at open (#379 §8); the
  vocabulary admits them, the destroy refuses them.
- **`volume show`, retention/GC policies, batch delete** — not this
  design.
- **Reaping tombstone rows** — *reopen if operator surfaces ever need
  to forget volumes entirely; DP1 records why not now.*

## 9. Residual risks

1. **This is the platform's first data-destroying primitive.** A bug
   in the guard chain or the locator deletes real operator data.
   Mitigations, layered: the attached/in-flight/schedule/kind guards
   at accept (DP5–DP7), ownership (DP5's `require_volume_owner` in-tx),
   the session refusal in stord (DP3), the safe-id traversal guard
   (DP4), the confirm dialog (DP12), and idempotent-not-destructive
   semantics for the absent-artifact case. The PR 2 description must
   carry the high-risk disclosure (CONTRIBUTING).
2. **The tombstone keeps the rows and the task history forever** — the
   operations table grows monotonically with deletes. Same posture as
   VM delete (accepted there since #406); a retention story is §8.
3. **The `'Deleting'` quota predicate (DP9) is a new WHERE clause on
   the canonical #525/#526 query** — its no-double-count invariant and
   its tests must be re-derived in the same PR (the query's own
   doc-comment demands exactly this).
4. **The stray attach-path file (§2.4) survives every delete** — a
   standalone local volume that was attached before deletion leaves
   `runtime_dir/{volume_id}` behind. Disclosed; the fix belongs to the
   attach surface; the destroy locator must NOT be "widened" to sweep
   it (DP4) because that would hard-code the bug.
5. **The sibling-verb `'Deleting'` guard (DP8) is one PR 2 predicate
   away from complete** — if review drops it, an attach raced against
   a delete can create-on-open a fresh file behind the tombstone.
   Pinned by a PR 2 test. [Corrected 2026-10-06, #535:] "one predicate
   away from complete" was optimistic — the BFF predicate is a
   non-transactional pre-check, and the CP's mutation journaling had
   no `'Deleting'` gate, so a concurrent attach landing after the
   delete's commit could still overwrite the tombstone (review S1).
   The folded backstop: the mutation-verb patch UPSERTs refuse
   `'Deleting'` rows at the SQL level (see DP5's correction); the
   in-flight guard also gained `RetryPending` (review S2 — a create
   in dispatch backoff would otherwise re-provision behind the
   tombstone on retry).
6. **No pre-#513 volume is deletable (DP6)** — every volume created
   before #513 is embedded lineage with NULL kind; operators cleaning
   legacy residue still have no sweep. Flagged as scope reality, not
   folded: the safe default refuses; a reclassification or
   embedded-delete design is the follow-up.
7. **LVM `lvremove` of an LV with snapshots fails at dispatch, not at
   accept** — the CP cannot see stord-side snapshots (§2.6). The
   failure is loud and terminal (safe direction), but it is an
   accept-then-failed case on this surface; disclosed rather than
   solved (§8).
8. **The freeze discipline (harness, not doc)** — if review renames
   anything here (route, key, rpc), the harness rows and this document
   must move in the same PR (#372 residual 5, verbatim).

---

## Appendix — suggested #522 issue-body corrections (scope)

1. **"Tombstone/journal semantics mirroring the VM-delete story
   (desired-state removal + physical reclaim, crash-safe between the
   two)" overstates the precedent:** the VM-delete story has NO
   physical reclaim (§2.2 — detach+close only, the hypervisor layer
   disclaims disk images). The crash-safe-between-two-phases property
   is new construction here (tombstone at accept, idempotent destroy
   at dispatch), not a mirror.
2. **"Node RPC (`DeleteVolume`) with agent-side reclaim per backend"
   understates the carrier:** the reclaim primitive must be a stord
   `DestroyVolume` rpc + `StorageBackend::destroy` method first (§2.3)
   — the agent cannot resolve the local path or safely bypass
   sessions. PR 1 is two layers, not one.
3. **Add the accept-time guards the issue does not name:** enabled
   backup schedules (DP7) and the attached-to-a-deleting-VM refinement
   (DP5) — the latter is required or volumes of deleted VMs are
   permanently undeletable.
4. **Record the `volume_kind` consequence:** only #513-era `'data'`
   volumes (and their clones) are deletable; every pre-#513 volume is
   refused (DP6) — the legacy-residue sweep remains a separate issue.
5. **Record the quota release mechanism** (DP9's one predicate on
   `storage_usage_bytes`) — without it, tombstoned volumes keep
   accruing against their owner's storage quota.
