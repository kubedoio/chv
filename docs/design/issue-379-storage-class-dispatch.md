# #379 design writeup — LVM storage class unreachable from the VM lifecycle (agent hardcodes backend class "local"; no volume-create API; LVM provisioning out-of-band)

**Issue:** kubedoio/chv#379 — the agent opens every VM volume with the
hardcoded backend class `"local"`, so a stord configured with
`backend_type = "lvm"` can never serve the VM lifecycle; volumes only
exist as implicit VM boot disks (no volume-create API); the LVM VG (and
its LVs) must be provisioned out-of-band by the operator. The LVM
backend is qualified dead code from a user's perspective
(`docs/evidence/production-readiness/v0.3.0-rc1/04-real-host-qualification/m4.5-storage.md:122-129`,
recorded as a design decision in
`m4.9-status.md:107`).

**Status:** FINAL — investigation + maintainer decision (§5.1, 2026-10-05):
all recommendations adopted, landed as the three-PR decomposition (§4).
PR 1 (agent class plumbing, #510) and PR 2 (CP carry, #511) are merged;
PR 3 (LVM enablement — DP2/DP4/DP5/DP7, this change) closes the
sequence. Evidence cited `file:line` at main `94e97b65` (verified
`git rev-parse HEAD`). This issue was sequenced after #385 (stord
respawn config fidelity, merged as #506) precisely so the control-plane
dispatch is not built on a respawn path that silently reverts backend
selection — see §2.6.

---

## 1. Problem statement

Three gaps stack to make LVM unreachable, and they are **not** one
problem:

1. **Dispatch gap (the "local" hardcode).** Every path that opens a
   volume on behalf of a VM — the legacy reconcile fan-out, the legacy
   re-attach loop, the Core executor, and the attach/fragment handler's
   default — sends backend class `"local"` (or a local-file-shaped
   locator) to stord. A stord running the LVM backend rejects the open
   (`LVMBackend::open` fails `BackendUnavailable` for any class but
   `"lvm"`, `crates/chv-stord-backends/src/lvm.rs:108-113`), so the VM
   lifecycle can never land on an LV.
2. **Provisioning gap (the create API).** On the local backend, volumes
   materialize *implicitly*: the first `open_volume` with
   `size_bytes`/`seed_from` options creates and sizes the backing file
   (`crates/chv-stord-backends/src/local.rs:266-349`). There is no
   CreateVolume RPC in the control-plane→node contract
   (`proto/controlplane/control-plane-node.proto:480-495` has
   CreateVm/AttachVolume/Resize/Snapshot/Clone — no CreateVolume), no
   BFF volume-create route (`crates/chv-webui-bff/src/router.rs:286-303`),
   and no `chvctl volume create`
   (`cmd/chvctl/src/commands/volume.rs:8-27` — List/Snapshot/Clone only).
   `LVMBackend::open` does **not** create LVs — provisioning is the
   operator's job by design
   (`crates/chv-stord-backends/tests/lvm_real.rs:11-13`).
3. **Out-of-band gap (operator model).** To get LVM storage today an
   operator configures `backend_type = "lvm"` +
   `lvm_volume_group = "<vg>"` in `stord.toml`
   (`crates/chv-config/src/lib.rs:245-260`;
   `cmd/chv-stord/src/main.rs:82-85`, VG default `"chv-vg"`), creates the
   VG out-of-band, and pre-creates LVs whose names equal the volume ids
   the control plane will mint — which the operator cannot know in
   advance, because the BFF mints volume ids at VM-create time
   (`crates/chv-webui-bff/src/handlers/vms.rs:511`). The m4.5
   qualification therefore exercises LVM only through root-gated
   stord-layer tests on a disposable loopback VG
   (`scripts/integration/qual/m4.5-storage.sh:808-872`), never through a
   VM.

The question for the maintainer is which of these to close, in what
order, and with which dispatch model — per-volume class selection,
node-level backend defaults, or a storage-class pool/scheduler — given
that the stord daemon is **single-backend per process** (§2.2) and that
#378 (core-managed fail-closed volume ops) and #372 (chvctl↔BFF contract
drift) both touch the same surfaces.

## 2. Ground truth at `94e97b65`

### 2.1 The hardcoded-"local" census, verified

Every production site that writes or assumes a backend class on the
VM-lifecycle path, classified: **(a)** must change for dispatch,
**(b)** must stay (default/validation), **(c)** cosmetic, dead, or
disconnected. Line refs at HEAD; the m4.5 evidence doc's refs
(`m4.5-storage.md:124-125` cites `reconcile.rs:934, :1134` at candidate
`b7e03e6f`) have drifted to `:949` / `:1149` at HEAD — the recorded
claims themselves remain accurate.

#### Class (a) — must change for dispatch (9 sites, 4 groups)

| # | Site | file:line | What it does today |
|---|---|---|---|
| A1 | Legacy VM-create volume open | `crates/chv-agent-core/src/reconcile.rs:1146-1154` | `open_volume_with_options(&disk.volume_id, "local", …)` inside `prepare_vm_resources`; locator is local-file-shaped (`vm_dir.join("{volume_id}.img")`, `:1139`) |
| A2 | Legacy reconcile re-attach loop | `reconcile.rs:938-977` (open at `:949`) | `open_volume(&volume_id, "local", &locator, …)` for previously-opened volumes; locator `format!("{}.img", volume_id)` (`:946`) |
| A3 | Core executor volume open | `crates/chv-agent-runtime-ch/src/core_runtime.rs:375-431` (open at `:401-409`) | `open_volume(volume_id, "local", …)`; locator `vm_dir.join("{volume_id}.img")` (`:385`); options mirror the legacy path (`size_bytes`/`seed_from`, `:389-400`) |
| A4 | Attach/fragment handler default | `crates/chv-agent-core/src/agent_server.rs:89-119` (`:96-105`) | `open_and_attach_volume` **already parses** `backend_class` and `locator` out of the volume `spec_json`, defaulting both (`unwrap_or("local")`, locator defaults to the bare `volume_id`). The parameterization half-exists; no producer ever sets the fields |
| A5 | CP VM-spec disk model | `crates/chv-controlplane-service/src/orchestrator.rs:1907-1913` (`AgentDiskSpec`) | Carries `volume_id`/`read_only`/`size_bytes` only; built at `:1570-1584` from a query that selects `volume_id, read_only, capacity_bytes` only (`:1527-1544`) — the class never leaves the store |
| A6 | Agent disk-spec model | `crates/chv-agent-core/src/spec.rs:31-38` (`DiskSpec`) | No class field (serde-tolerant: unknown fields are ignored, so an additive field is backward-compatible) |
| A7 | Core definition model | `crates/cellhv-core-types/src/lib.rs:354-365` (`StorageAttachmentRef`) | `attachment_id`/`storage_ref`/`read_only`/`size_bytes`/`seed_from` — no class; Core cannot express "this disk is LVM" |
| A8 | CP→agent attach dispatch, empty spec | `crates/chv-controlplane-service/src/node_client.rs:649-671` (`volume_spec_json: vec![]` at `:670`) | The AttachVolume RPC's `VolumeMutationSpec.volume_spec_json` is always empty — A4's parser always takes the defaults *(corrected below, 2026-10-05)* |
| A9 | BFF attach mutation, empty spec | `crates/chv-controlplane-service/src/bff_mutations.rs:422-433` (`:432`) | Same: the BFF's `mutate_volume(action="attach")` sends `volume_spec_json: vec![]` |
| A10 | Agent `CreateVm` RPC, legacy branch | `crates/chv-agent-core/src/agent_server.rs:1001-1120` (open at `:1097`) | `create_vm`'s non-core branch opens every disk with the literal `"local"` and a `{volume_id}.img` locator — a fifth class-writing open site. Not reachable from the current control plane (verified: `node_client::create_vm` (`node_client.rs:369`) has no production caller; the CP drives VM create through the gateway → desired-state persist → the A1/A2 reconcile fan-out, and core-managed nodes go through the authority → A3), so functionally a dead RPC today — but it is a live class-writing open that PR 1's plumbing must cover (or explicitly freeze), and it is the surface DP6's recommended `CreateVolume` RPC will mirror |

#### Class (b) — must stay (defaults and validations)

| # | Site | file:line | Why it stays |
|---|---|---|---|
| B1 | stord backend default | `cmd/chv-stord/src/main.rs:53` | `backend_type` absent → `"local"`; the node-level default contract |
| B2 | stord fallback arm | `main.rs:86` | unknown `backend_type` → `LocalFileBackend`. Existing behavior, and the default contract for B1 must stay — but the *unknown-value* arm should not stay silent: with #379 making `backend_type` a mainline dispatch input, a typo'd `backend_type = "lvv"` on an operator's LVM intent would silently serve local-file. Folded into DP2/PR 3 as an explicit decision (fail closed on unrecognized `backend_type`), not left as an accident |
| B3 | Local backend class validation | `crates/chv-stord-backends/src/local.rs:244-249` | accepts `local`/`local-file`/`localdisk` — the alias surface |
| B4 | LVM/ceph/iscsi class validations | `lvm.rs:108-113`, `ceph.rs:147-149`, `iscsi.rs:409-411` | per-backend class checks — the *actual* dispatch enforcement point (§2.2) |
| B5 | Agent default on empty spec | `agent_server.rs:101` | `unwrap_or("local")` — the back-compat default once a producer exists |
| B6 | Path-allowlist scoping | `crates/chv-stord-core/src/handlers.rs:395-412` | path allowlist applies to local-class locators only (lvm/iscsi/ceph locators are identifiers, not paths) — correct as-is |

#### Class (c) — cosmetic, dead, or disconnected

| # | Site | file:line | State |
|---|---|---|---|
| C1 | Local backend self-labels | `local.rs` (42 `"local"` literals before the test module at `:1087`) | error `backend:` labels and export metadata — correct self-labeling, no action |
| C2 | chvctl storage create doc | `cmd/chvctl/src/commands/storage.rs:20` | documents `"local", "ceph", "iscsi"` — omits `lvm` (cosmetic; the command itself is dead, §2.6) |
| C3 | Agent inventory probe | `crates/chv-agent-core/src/inventory.rs:94-102` (`:128`) | `KNOWN = ["localdisk", "ceph", "nfs"]` probed as *directories* under `storage_base_dir` — misses `lvm`, includes `nfs` (not a stord backend), and never consults stord's `backend_type` |
| C4 | `storage_pools` catalog | migration `0015_storage_pools.sql:1-12`; BFF `handlers/storage.rs:63-130`; CP stub `api/stub.rs:23-99`, `api/router.rs:128-131` | a UI-facing catalog table with a create route (`backend_class` default `'localdisk'`, `0015:5`) that nothing in provisioning reads or writes — see §2.5 |
| C5 | BFF volume display | `handlers/volumes.rs:135`, `:202`, `:483` | `storage_class` is surfaced (`COALESCE(v.storage_class,'')`) and the proto viewmodel carries it (`proto/webui/webui-bff.proto:239`) — displays the empty string today |
| C6 | UI types | `ui/src/lib/api/types.ts:55-73` | `CreateStoragePoolInput.pool_type: 'localdisk'` only; the volumes UI has no create surface (list + detail pages only) |

**Store-model truth (premise correction).** The issue's phrase "volume
models hardcode backend class 'local'" is imprecise in a useful way:
the `volumes.storage_class` column is nullable
(`cmd/chv-controlplane/migrations/0001_initial.sql:122-131`) and every
production writer leaves it **NULL** — the BFF's VM-create/import/
template INSERTs omit it (`vms.rs:602-616`, `imports.rs:218-231`,
`templates.rs:419-433`), and the clone path copies the source's value
(`crates/chv-controlplane-store/src/desired_state.rs:857` — all
`desired_state.rs` citations in this doc are the store crate's
`chv-controlplane-store/src/desired_state.rs`). Nothing stores the
string `"local"`. The
hardcode lives entirely in the **agent dispatch** (A1–A4, A10) plus the
**spec models' missing field** (A5–A7). That is good news for
migration: "implicit NULL = local" is already the de-facto contract,
and the fragment path already round-trips a class end-to-end
(`VolumeSpec.storage_class`, `crates/chv-controlplane-types/src/fragment.rs:42-55`,
applied at `crates/chv-controlplane-service/src/reconcile.rs:478`) —
there is simply no producer.

**Census correction (2026-10-05, PR 3).** The A8 row's "A4's parser
always takes the defaults" was inaccurate at the letter, and PR 2's
merge notes disclosed the discrepancy: `serde_json::from_slice` on the
producer's **empty** `volume_spec_json` is an EOF *error*, not a
defaults-taking parse — the NULL-class attach spec never parsed at A4
(the dispatch worked only because the failure was never exercised with
a non-default class in mind). PR 2 preserved the empty bytes
byte-exactly (deliberately: PR boundaries don't change wire formats);
PR 3 fixes the producer to emit `b"{}"` — a spec that parses and takes
every default, which is what this census row always described. The
A4/A9 contract-pair tests pin the new shape, and the CHANGELOG
discloses the wire-format change (an empty-byte spec is now `{"…"}`
rather than empty).

### 2.2 stord is single-backend per daemon — the locator class is a check, not a router

The `BackendLocator.backend_class` field
(`proto/node/chv-stord-api.proto:28-32`) reads like per-volume routing,
but `chv-stord` constructs exactly **one** backend at startup from
`backend_type` (`main.rs:53-87`) and every backend rejects foreign
classes (B3/B4). So:

- **Within a node**, per-volume backend selection is impossible today:
  all volumes on a node share the daemon's backend. A VM asking for
  "one local disk and one LVM disk" on the same node cannot be served
  by the current stord.
- **Across nodes**, per-volume selection is really *placement*: choose
  a node whose stord runs the requested class. The control plane
  already places VMs and journals clones on the source's node (#381
  lineage, `desired_state.rs:463-464`).
- The allowlists apply at this boundary too: `backend_allowlist` gates
  the class (`handlers.rs:193-204`, call site `:385`), the path allowlist
  is local-only (B6), and `device_allowlist` is checked for
  `lvm`/`block` classes against the **raw locator string**
  (`handlers.rs:414-424`; glob match `:310-318`). The standard install
  writes `device_allowlist = ["/dev/dm-*", "/dev/mapper/*"]`
  (`scripts/install.sh:1036`) — an LVM open whose locator is the bare
  volume id would be denied under that default. Note the LVM backend
  *ignores* the locator and derives `/dev/{vg}/{vid}` from the
  sanitized volume id (`lvm.rs:54-60`, `:114-120`;
  `lvm_real.rs:53-58` uses `"vg/ignored-by-lvm-backend"`), so the
  locator for LVM opens is purely an allowlist token and its convention
  must be defined by whatever dispatch design is chosen (§5 DP5). Be
  explicit about what that means: `device_allowlist` gates the
  **locator token only** — the device the guest actually receives is
  the backend-derived `/dev/{vg}/{vid}`, which is not allowlist-checked
  (the class check and `is_safe_id` sanitization are the real
  constraints on it); do not over-credit the allowlist as constraining
  the accessed device.

### 2.3 How volumes come into existence today (the create-API gap, verified)

| Route | Row creation | Backing-store creation | storage_class |
|---|---|---|---|
| BFF VM create | `INSERT INTO volumes` in the create tx (`vms.rs:602-616`; id minted `:511` per #384) | agent `open_volume` create-on-open with `size_bytes`/`seed_from` options (`reconcile.rs:1126-1154` → `local.rs:266-349`) | NULL |
| BFF VM import | `imports.rs:218-231` | same, seeded from the imported file | NULL |
| BFF template instantiate | `templates.rs:419-433` | same | NULL |
| Clone (#501) | `materialize_clone_target`, one tx, strict insert (`desired_state.rs:824-939`) | stord `prepare_clone` on the source's backend (`agent_server.rs:1903-1948`, fail-closed in core-managed mode) | copied from source (`:857`; replay-shape match compares it, `:888`) |
| CP fragment apply | `upsert_volume` (`desired_state.rs:618-667`, SQL `:76-110`) via `ApplyVolumeDesiredState` (`reconcile.rs:443-500`) | none — fragments are desired-state records | from fragment spec |
| Volume-create API | **does not exist** — no RPC (`control-plane-node.proto:480-495`), no BFF route (`router.rs:286-303`), no chvctl command (`volume.rs:8-27`) | — | — |

Two facts sharpen the "no volume-create API" premise:

1. **The provisioning primitive exists at the backend layer, scoped to
   migration receive.** The `StorageBackend` trait has
   `create_receiving_volume` — `lvcreate -L <size> -n <vid> <vg>` on
   LVM (`lvm.rs:662-703`), file creation on local (`local.rs:994`) —
   used only by the disk-migration receiver. A first-class create path
   would not be inventing new backend capability; it would be exposing
   an existing one (or extending `open` to create-on-open for LVM, see
   §4 DP2).
2. **The only production CP→agent volume-fragment push is the
   migration dirty-tracking cleanup** (`crates/chv-controlplane-service/src/migration.rs:918-935`),
   whose spec `{"dirty_tracking": false}` carries no class, no vm_id,
   no locator. The fragment channel (`ApplyVolumeDesiredState`,
   `node_client.rs:273-319`) is otherwise unused in production — it is
   available as a dispatch carrier without proto changes.

### 2.4 LVM provisioning out-of-band, verified (operator model)

What an operator does today, and what each layer knows:

- **stord constructor inputs:** `backend_type = "lvm"` +
  `lvm_volume_group` (default `"chv-vg"`, `main.rs:82-85`;
  `chv-config/src/lib.rs:257-260`), plus the standard
  `device_allowlist` (`install.sh:1036`, `docs/examples/stord.toml:8`).
  `LVMBackend::new` only sanitizes the VG name (`lvm.rs:26-33`) — it
  does **not** verify the VG exists at startup (health does, per-LV:
  `lvm.rs:185-195`); a bad VG name surfaces as unhealthy volumes, not
  a startup error.
- **What the m4.5 scenario does** (the only exercised LVM path):
  provisions a loopback PV → disposable VG `chvqual-m45` out-of-band
  (`m4.5-storage.sh:808-872`), runs the 7 root-gated `lvm_real.rs`
  tests (each provisioning and removing its own LVs, `lvm_real.rs:11-18`),
  and asserts zero residue. Run 10: 7/7
  (`m4.9-status.md:40`). **Capability label: LVM is qualified at the
  stord layer only** — real loopback VG/LV, block write/read through
  `/dev/{vg}/{vid}`, COW snapshot/clone, `lvresize`, read-only policy,
  cleanup (m4.5-storage.md §3 Leg F, `:42`, `:61`).
- **What the agent/control plane would need to know:** VG existence
  (currently unknowable — stord exposes no status/config RPC; #385
  §2.5 recorded that gap), capacity (nothing reports free extents; the
  `storage_pools.total_bytes`/`used_bytes` columns are never populated
  by anything but the BFF create route), and placement policy (which
  node offers which class — see §2.5).
- **What the agent does know:** nothing about stord's backend. It does
  not read `stord.toml` except the supervisor's respawn validation,
  which parses it when `stord_config_path` is set
  (`crates/chv-agent-core/src/supervisor.rs:88-95`) — an existing,
  validated acquisition route for the node's `backend_type` if Option C
  (§4) is chosen.

### 2.5 The phantom surfaces (found during investigation)

Three plumbing layers *look* like they already implement a storage-class
pool model but are mutually inconsistent and dead end-to-end:

1. **`NodeInventory.storage_classes`** (`proto/controlplane/control-plane-node.proto:30`)
   is populated by directory probing (C3) and persisted as a JSON array
   of **strings** (`crates/chv-controlplane-service/src/enrollment.rs:191-195`,
   `inventory.rs:108-114`).
2. **The architecture-reconcile fleet provider** consumes that blob but
   expects an array of **objects** `{name, kind, capacity_gb, free_gb}`
   (`crates/chv-architecture-reconcile/src/fleet_inventory.rs:116-186`,
   parse at `:157-170`) — string items never yield a `name`, so
   `list_datastores` always returns empty. The datastores surface is
   dead code as operated.
   [Corrected 2026-10-07, #546:] the surface is live — `list_datastores`
   now parses the string array enrollment actually persists (each class
   string yields an entry named by the class, `capacity_gb`/`free_gb`
   unknown — `Option`, never fabricated; the object shape keeps
   parsing), class-named entries suppress `DATASTORE_NOT_FOUND` for
   classes a node offers, and the `DATASTORE_INSUFFICIENT_CAPACITY`
   check downgrades to a warning on unknown capacity (`Some(0)` still
   blocks).
3. **The `storage_pools` table + BFF/CP create routes** (C4) accept an
   operator-invented `pool_type` string with operator-supplied
   capacity, connected to no provisioning, no placement, and no stord
   configuration.

Any "storage-class pool" design (Option B, §4) is building on plumbing
that must first be made truthful — or deleted.

### 2.6 Ordering with #385, #378, #372

- **#385 (landed, #506):** respawn now preserves `backend_type` for
  deployments that set `AgentConfig.stord_config_path`
  (`chv-config/src/lib.rs:475-489`; `install.sh:1026` sets it for
  standard installs). The #385 design doc's §2.6 premise-sharpening
  (`docs/design/issue-385-stord-respawn-config-fidelity.md:210-233`)
  called this ordering out: #379 makes `backend_type` a mainline
  dispatch input, and operating that on a respawn path that discards it
  converts an edge-case availability break into a guaranteed one.
  **Residual:** the qual topology's `deploy.sh` is deliberately not
  wired (#385 §7/§9.7; verified — no `stord_config_path` in
  `scripts/integration/qual/deploy.sh` at HEAD), so a stord crash
  during an LVM qual run still respawns local via the generated config.
  Any #379 enablement that flips the qual topology to LVM must wire
  `deploy.sh` in the same change (§5 DP7).
- **#378 (open, accept-time rejection landed via #495):** snapshot/
  clone/restore dispatch fails closed on core-managed nodes, and the
  lifecycle now rejects at accept time via
  `ensure_volume_snapshot_family_supported`/`ensure_node_not_core_managed`
  (`crates/chv-controlplane-service/src/lifecycle.rs:363-393`). This is
  the natural shape for a storage-class capability check ("node does
  not offer class X") — same repository, same accept-time discipline.
  Core itself does not model volume ops; `StorageAttachmentRef` gaining
  a class field (A7) is additive and does not unlock execution by
  itself.
- **#372 (queued after #379):** the chvctl↔BFF contract drift already
  includes a storage-surface casualty: `chvctl storage create` POSTs
  `/v1/storage/pools` with field `backend`
  (`cmd/chvctl/src/commands/storage.rs:59-68`, body `:64`) while the BFF
  exposes `POST /v1/storage-pools/create` with `pool_type`/
  `backend_class` (`handlers/storage.rs:80-85`; routes
  `router.rs:155-156`, `:306-307`) — the command is dead-on-arrival
  today. **Overlap note (not designed here):** any volume-create API
  from #379 adds a route + field vocabulary on the same BFF/chvctl
  surface #372 plans to reconcile. The two issues should agree on field
  names and route shapes once, in #372's contract pass, rather than
  churning the surface twice (§5 DP6).

## 3. Goals & non-goals

**Goals**

- A VM disk can be placed on LVM storage through the normal lifecycle
  (create/attach/restart/recover), with the LVM path qualified at the
  same tier as the local path is today (real-VG qual, not just stord
  unit tests).
- Existing volumes — every `storage_class` NULL row — keep working
  byte-exactly: same locators, same files, same open sequence.
- Deployments that never configure LVM see no behavior change at all.
- The #501 clone transaction's source-read shaping and replay-match
  (which compares `storage_class`, `desired_state.rs:888`) keep
  holding.
- The dispatch design does not silently depend on a respawn path that
  reverts backend selection (#385's ordering premise).

**Non-goals** (§8 restates with reopen triggers): iscsi/ceph classes;
capacity-aware scheduling; mixed-backend nodes (stord stays
single-backend per daemon); live backend migration of existing volumes;
#378's Core execution modeling; #372's contract reconciliation; making
the `storage_pools` catalog truthful (unless DP4 opts in).

## 4. Options

### Option A — per-volume backend class, carried end-to-end (the volume-model design the evidence doc names)

**Shape.** `volumes.storage_class` becomes the semantic field it always
pretended to be: the BFF's VM-create payload (and any future
volume-create API) accepts an optional class, default NULL = local. The
CP carries it through `AgentDiskSpec` (A5) → agent `DiskSpec` (A6) →
Core `StorageAttachmentRef` (A7), and the agent's five open sites (A1–
A4, A10) use it instead of the `"local"` literal, keeping `"local"` as the
absent-field default (B5). Locator shaping becomes class-dependent
(local: `{volume_id}.img` under the VM dir; lvm: a defined allowlist
token — DP5). The attach path (A8/A9) populates `volume_spec_json`
with `{"backend_class": …, "locator": …}` — A4's parser already reads
exactly that, so the attach dispatch needs no proto change. Accept-time
validation reuses the #495 shape: reject a class the node does not
advertise (DP4).

**What changes:** ~9 census sites across 5 crates, one additive serde
field in three structs, BFF payload plumbing, an accept-time check.
**What breaks:** nothing for NULL classes (the default path is
unchanged by construction); the clone path already copies the class
(`:857`) so clones inherit their source's class for free. **The gap it
does NOT close:** provisioning (§2.3) — an LVM-class VM disk still
needs an LV to exist, so A requires DP2 (create-on-open parity or an
explicit create API). **Security surface:** LVM opens route through
`device_allowlist` on the locator string (§2.2) — the class field must
come with a locator convention that the standard install's allowlist
admits, or the default posture denies every LVM open. **Test burden:**
moderate — unit pins at agent and CP tiers, plus one new qual leg
(DP7).

### Option B — node/storage-class pool model (advertise + schedule)

**Shape.** Make §2.5's phantom plumbing real: the agent reports the
node's actual backend (from `stord_config_path`'s parsed
`backend_type`, or a new `AgentConfig` key — not directory probing) as
`NodeInventory.storage_classes`; the CP persists it (the column already
exists), placement consults it when a VM or volume names a class; the
`storage_pools` catalog (or its replacement) becomes the operator-facing
view with capacity from `vgs` (needs a stord status RPC or agent-side
probe).

**What changes:** everything in A *plus* the inventory truth-fix, a
placement policy hook, capacity reporting, and either deleting or
repairing the fleet datastores parse. [Corrected 2026-10-07, #546:]
the parse half of that repair has landed — see §2.5 item 2's
correction; what remains deferred to B is the capacity half
(free-extent reporting). **What breaks:** nothing
existing — but the current probe is actively wrong on LVM nodes:
`install.sh:224` creates `storage/localdisk` and `storage/lvm`, the
`KNOWN` list probes only `localdisk`/`ceph`/`nfs` (`inventory.rs:96`),
so a node running `backend_type = "lvm"` advertises `["localdisk"]`
today (§2.5). **Tradeoff paragraph:** B is the
"right" fleet answer and the natural prerequisite for multi-node
class-aware placement, but it front-loads a scheduler design and a
capacity-reporting surface that no current deployment needs — the
platform is single-node-per-VM today, clone placement already follows
the source's node, and every existing deployment has exactly one
backend per node. It also has to decide the fate of three dead surfaces
(storage_pools, fleet datastores, the probe) — each a small design
decision of its own. [Corrected 2026-10-07, #546:] the fleet
datastores member of that list is no longer dead (see §2.5 item 2's
correction); `storage_pools` and the probe remain as stated. B should
be the *destination*, not the first step.

### Option C — config-driven node-level default (the minimal-change option)

**Shape.** Accept that stord's `backend_type` already selects the
backend, and that the *only* thing broken is the agent asserting
`"local"`. The agent learns the node's backend class once (from the
`stord.toml` it already parses for respawn validation,
`supervisor.rs:88-95`, or a new `AgentConfig` key) and uses it at the
A1–A4/A10 open sites. The control plane stays ignorant: no spec changes, no
BFF changes, volume rows keep NULL classes, the UI shows nothing new.

**What changes:** one agent crate. **What breaks:** nothing — absent
config, the class is `"local"` exactly as today. **Honest costs:**
(i) node-uniform — every volume on the node is LVM, local-file VMs on
the same node become impossible (they'd fail class validation, B3);
(ii) the volume rows lie by omission (NULL reads as local in the BFF
display, C5); (iii) multi-node placement is blind to the class; (iv)
the provisioning gap is **not** dodged — VM create on an LVM node still
needs LV creation, and since LVs cannot be pre-named after BFF-minted
ids (§1), C *still* needs DP2. **Tradeoff paragraph:** C is the
smallest change that makes LVM reachable, and it is genuinely
attractive as a first landing — but it makes "storage class" a node
property the control plane cannot see, which collides with the issue's
own framing (the BFF already displays a per-volume `storage_class`
field, C5) and with clone placement on multi-class fleets. It is best
understood as the enablement core of A (the agent-side class plumbing)
with the CP-side carry deferred — not as a competing end-state.

### The decomposition the options imply

A and C share the same agent-side core (stop hardcoding; class-aware
locator/provisioning). The separable pieces, in dependency order:

1. **PR 1 — agent class plumbing (C's core, A's A1–A4/A10/B5):** the agent
   uses a class value instead of the literal, defaulting to `"local"`;
   no producer yet; zero behavior change. Pin with unit tests at the
   agent tier.
2. **PR 2 — CP carry (A's A5–A9):** spec models + attach spec_json
   population + BFF payload field; NULL still means local everywhere.
3. **PR 3 — LVM enablement:** DP2's provisioning hook, DP4's
   accept-time capability check, DP5's locator/allowlist contract,
   deploy.sh wiring (DP7), and the qual leg.
4. **Volume-create API (separable, DP6):** a standalone create surface
   is *not required* for LVM dispatch — VM-create-embedded volumes can
   carry the class — but it is the natural home for standalone data
   volumes and should be designed with #372 in one pass.

## 5. Decision points (maintainer)

**DP1 — dispatch model: A (per-volume class) vs C (node-level default)
vs B (pool/scheduler).** *Recommendation: A, landed as the §4
decomposition (PR 1–3); treat C as PR 1's scope, not an end-state; B
deferred with reopen triggers (§8).* A matches the data model that
already exists (nullable `storage_class`, BFF display field, fragment
round-trip), keeps NULL = local byte-exact, and does not preclude B —
B consumes A's field. C alone leaves the volume rows lying and blocks
mixed fleets; B alone is a scheduler built before any volume can carry
a class.

**DP2 — the LVM provisioning hook (the create gap).** *Recommendation:
create-on-open parity on LVM* — extend `LVMBackend::open` (or add a
first-class `create_volume` trait method) to `lvcreate -L <size> -n
<vid> <vg>` when the LV is absent and `size_bytes` is present, reusing
`create_receiving_volume`'s exact shape (`lvm.rs:662-703`); seed_from
is out of scope for v1 (LVM has no seed path; document it). This makes
VM-create work with zero API additions and mirrors local semantics
(`local.rs:266-349`). *Alternatives:* an explicit CreateVolume RPC
(more contract, enables standalone volumes — fold into DP6); or the
operator pre-provisions LVs (rejected: ids are minted at BFF time,
§1). Whichever is chosen, `LVMBackend::new` should also fail closed at
startup on a missing VG (it currently doesn't, §2.4) — a one-line
`vgs` check in the `main.rs:82-85` arm — and the unrecognized-
`backend_type` fallback arm (`main.rs:86`, B2) should fail closed at
startup in the same PR: `backend_type` becomes a mainline dispatch
input under #379, so a typo'd value silently serving local-file is a
fail-open footgun on exactly the path this issue enables (absent key
still means local, B1 — only *present-but-unknown* values abort).

**DP3 — vocabulary.** Three vocabularies exist: stord `backend_type`
{local, iscsi, ceph, lvm} (`chv-config/src/lib.rs:245`), locator
classes {local, local-file, localdisk, lvm, block, iscsi, ceph, rbd}
(`local.rs:244-246`, `handlers.rs:401`, `:414`), and inventory
{localdisk, ceph, nfs} (`inventory.rs:96`). *Recommendation: the
volume-model field uses the stord `backend_type` vocabulary with
`"local"` as the canonical default; the local aliases
(`local-file`/`localdisk`) remain accepted at the stord boundary only;
`block` is documented as an alias of `lvm` for the device-allowlist
branch. Freeze this in DP6/#372 so chvctl, BFF, and the agent never
diverge again.*

**DP4 — capability reporting + accept-time rejection.** The agent
should report the node's actual backend class in
`NodeInventory.storage_classes` (source: the parsed `stord_config_path`
or a new `AgentConfig` key — not directory probing, which reports
`localdisk` on LVM nodes, §2.5), and the lifecycle should reject a
volume/VM create naming a class the node doesn't offer, reusing the
#495 `ensure_*` shape (`lifecycle.rs:363-393`). *Recommendation: do
the reporting + accept-time check in PR 3; defer capacity reporting
(free extents) and the fleet-datastores repair to B with reopen
triggers.*

**DP5 — LVM locator + allowlist contract.** The agent must send a
locator string for LVM opens that (i) the LVM backend can ignore (it
derives the path itself) and (ii) the standard `device_allowlist`
admits. *Recommendation: locator = `/dev/mapper/{vg}-{vid}`-shaped dm
path (matches the standard install's `/dev/dm-*`/`/dev/mapper/*`
patterns, `install.sh:1036`), with the VG learned from the same source
as DP4; `is_safe_id` on the volume id (already enforced at the agent
boundary, `reconcile.rs:1117`, and re-sanitized in the backend,
`lvm.rs:35-52`) keeps the LV-name component safe.* The alternative
(bare volume id) silently denies every LVM open on standard installs.

**DP6 — volume-create API: in-scope vs separate.** *Recommendation:
separate issue, designed jointly with #372's contract pass.* It is not
required for LVM dispatch (DP2 + the class carry suffice for
VM-embedded disks); it *is* required for standalone volumes; and its
route/field surface is exactly what #372 will reconcile (§2.6). If the
maintainer wants it now, the minimal shape is: `CreateVolume` RPC on
the node LifecycleService (mirroring `CreateVmRequest`,
`control-plane-node.proto:196-200`), a BFF `/v1/volumes/create` route,
and `chvctl volume create` — with field names fixed per DP3.

**DP7 — qual surface + deploy.sh wiring.** *Recommendation:* extend
m4.5 with a Leg G (VM-integrated LVM: create → attach → guest
write/read → restart → recover on the loopback VG, reusing Leg F's
provisioning), wire `deploy.sh`'s `stord_config_path` (and the LVM
stord.toml) in the same PR — and give the qual `stord.toml` the
standard `device_allowlist = ["/dev/dm-*", "/dev/mapper/*"]` in the
same change, so Leg G exercises DP5's locator/allowlist contract
end-to-end against the realistic posture (the qual daemon today sets
no `device_allowlist` at all, `deploy.sh:430-434`, which would leave
the sole reason the locator convention exists untested at the tier
where the capability label upgrades) — then re-run the scenario. This
is the point where the LVM capability label upgrades from "stord layer
only" to "VM-integrated" in m4.9's declaration (`m4.9-status.md:40`,
`:316-317`).

**DP8 — core-managed mode posture.** On core-managed nodes, volume
snapshot/clone dispatch fails closed (#378); a *class-carrying*
`StorageAttachmentRef` (A7) is additive and does not change that. The
LVM *open* path is part of VM create, which Core executes — so Core
create on an LVM-class disk will dispatch through the runtime's
`open_volume` (A3) and work, while snapshot/clone stay fail-closed
exactly as today. *Recommendation: keep #378's posture untouched; add
the A7 field now so Core's model is ready; note in #378 that LVM does
not worsen it.*

### 5.1 Decision (maintainer, 2026-10-05)

All recommendations adopted, as written:

1. **DP1 — dispatch model: A (per-volume class), landed as the three-PR
   decomposition.** PR 1 (agent class plumbing), PR 2 (CP carry), PR 3
   (LVM enablement); C is understood as PR 1's scope, not an end-state;
   B (pool/scheduler) is deferred with the §8 reopen triggers.
2. **DP2 — provisioning hook: LVM create-on-open parity**, reusing
   `create_receiving_volume`'s exact shape; seed_from stays out of
   scope for v1. `LVMBackend::new` fails closed at startup on a
   missing VG, and the unrecognized-`backend_type` fallback arm
   (`main.rs:86`, B2) fails closed at startup in the same PR — a
   present-but-unknown value aborts; an absent key still means local
   (B1).
3. **DP3 — vocabulary:** the volume-model field uses the stord
   `backend_type` vocabulary with `"local"` as the canonical default;
   local aliases accepted at the stord boundary only; `block`
   documented as an `lvm` alias for the device-allowlist branch;
   vocabulary frozen jointly with #372.
4. **DP4 — capability reporting + accept-time rejection** in PR 3: the
   agent reports the node's actual backend class (from the parsed
   stord config, not directory probing), and the lifecycle rejects a
   class the node doesn't offer (the #495 `ensure_*` shape). Capacity
   reporting and the fleet-datastores repair deferred to B.
5. **DP5 — LVM locator contract:** a `/dev/mapper/{vg}-{vid}`-shaped
   dm path, with the VG learned from the same source as DP4.
6. **DP6 — volume-create API: separate issue, designed jointly with
   #372.** Not required for LVM dispatch.
7. **DP7 — qual surface:** m4.5 gains Leg G (VM-integrated LVM
   end-to-end); `deploy.sh`'s `stord_config_path` is wired and the
   qual `stord.toml` gains the standard `device_allowlist` in the same
   change; the scenario is re-run.
8. **DP8 — core-managed posture:** #378's fail-closed boundary stays
   untouched; the A7 class field lands in PR 2 of the decomposition
   (as `Option` + `#[serde(default)]`/`skip_serializing_if`, per §7's
   `deny_unknown_fields` caveat) so Core's model is ready.

## 6. Test strategy (per decision)

Existing pins to keep green (all at HEAD `94e97b65`):

- **Store tier:** the #384/#501 clone suite — strict-insert conflict,
  idempotent replay shape-match including `storage_class`
  (`desired_state.rs:888`), stale-generation rollback
  (`chv-controlplane-store/src/tests.rs`, #384 §6); the clone
  class-propagation tests
  (`chv-controlplane-service/src/tests.rs:3805-3849`).
- **Agent tier:** the volume reconcile/re-attach tests around
  `reconcile.rs:916-1078`; the attach/fragment handler tests
  (`agent_server.rs`'s test module `:3089+`, the attach/fragment
  spec_json pins at `~:3965`, including the stale-generation and
  core-managed fail-closed pins); the Core runtime's
  `open_and_attach_volume` tests via the deterministic mock
  (`crates/chv-agent-runtime-ch/src/mock.rs:83`).
- **stord tier:** `lvm_real.rs` 7/7 (root-gated, real VG — the
  qualified contract this issue builds on), the local backend's
  create-on-open tests, the allowlist handler tests
  (`handlers.rs:1355+`).
- **Qual:** m4.5 legs A–F (`m4.5-storage.md:36-42`) — Leg A/C pin the
  local create-on-open and respawn-recovery behavior that PR 1–2 must
  not disturb; Leg D pins the #378 fail-closed boundary DP8 relies on.

New tests per piece:

- **PR 1 (agent class plumbing):** unit — every class-writing open
  site (A1–A4, A10) receives the class from config/spec and defaults
  to `"local"` when absent;
  red/green: reverting to the literal must fail a test that drives a
  fake stord client asserting the received `backend_class`.
- **PR 2 (CP carry):** CP-tier — `build_agent_vm_spec` embeds the
  volume's class (NULL → field absent → agent default); BFF-tier —
  VM-create payload with/without class; the attach mutation populates
  `volume_spec_json` (A9) with the parsed class; round-trip with the
  agent-tier A4 parser (a contract pair test, mock tier).
- **PR 3 (LVM enablement):** stord-tier — LVM `open` creates the LV
  when absent + `size_bytes` present (loopback VG, root-gated, joins
  `lvm_real.rs`); refuses when absent without size; startup fails
  closed on a missing VG and on an unrecognized `backend_type` (B2). Agent-tier — LVM locator shaping (DP5)
  passes the standard device-allowlist fixture. CP-tier — accept-time
  rejection when the node's advertised class doesn't match (#495
  shape). Qual — Leg G as DP7.
- **If DP6 is taken:** store/BFF/chvctl triple for the create surface,
  with field names pinned per DP3, and an explicit #372-collision note
  in the PR description.

## 7. Rollout & rollback

- **PR 1–2 (class plumbing, no producer semantics):** behavior-neutral
  by construction (absent field = `"local"` = today's literal). Rollback
  is revert; no state, no contract change beyond additive serde fields
  (old agents ignore unknown fields — pinned by
  `fragment.rs:112-117`'s forward-compat tests, which cover the CP-side
  `VmSpec`; the agent's `spec.rs` models are likewise serde-tolerant —
  no `deny_unknown_fields`). One exception to state plainly: Core's
  `StorageAttachmentRef` (A7) carries
  `#[serde(deny_unknown_fields)]` (`crates/cellhv-core-types/src/lib.rs:353`),
  so its class field must land as `Option` +
  `#[serde(default)]`/`skip_serializing_if` — a bare additive field
  would make a mixed-version fleet reject the whole Core definition.
  Within this repo's same-commit deployment model (CP + agent ship
  together) that is belt-and-braces, but it is the one serde surface
  where "additive is back-compat" is not automatic.
- **PR 3 (LVM enablement):** gated on operator configuration
  end-to-end — nothing dispatches LVM unless (i) an operator sets
  `backend_type = "lvm"` in `stord.toml` (with `stord_config_path` set
  so respawns preserve it, §2.6) and (ii) a volume explicitly carries
  the class. Existing NULL-class volumes and never-LVM deployments are
  untouched. Rollback: revert; LVM-class volumes created meanwhile are
  ordinary LVs (operator-managed residue, same class as Leg F's
  out-of-band model). One durable effect to disclose: LVs created by
  create-on-open are not tracked for cleanup beyond the existing
  M2.5 volume-retention posture (`m4.5-storage.md:223-224`) — VM delete
  retains the volume row and, on LVM, the LV.
- **Documentation:** `docs/OPERATIONS.md` gains the LVM node contract
  (VG pre-provisioning, `device_allowlist` interplay, the DP5 locator
  convention); m4.9's declaration language upgrades with DP7's run.

## 8. Non-goals / scope boundaries (with reopen triggers)

- **iscsi/ceph as VM-attached classes** — the backends exist and
  validate their classes (B4) but are unqualified even at the stord
  layer (m4.5 §6: "not exercised, not claimed"). *Reopen when either
  backend gains a qualification run.*
- **Mixed-backend nodes** (one stord serving local and LVM volumes) —
  requires multi-backend stord, a new daemon design. *Reopen when a
  deployment need survives DP1's per-node-uniform model.*
- **Capacity-aware scheduling / fleet datastores repair** (Option B's
  upper half) — free-extent reporting, `vgs` polling, the
  fleet_inventory parse fix. *Reopen when multi-node class-aware
  placement is requested, or when the architecture designer's
  DATASTORE_NOT_FOUND checks stop being vacuous (§2.5).*
- **Live backend migration** of existing NULL-class volumes to LVM.
  *Reopen only with an explicit data-motion design (the stord
  migration machinery is the natural carrier).*
- **#378's Core execution modeling** for volume ops — adjacent, not
  load-bearing here (DP8).
- **#372's chvctl↔BFF reconciliation** — including the already-dead
  `chvctl storage create` (§2.6). #379 only commits to DP3's shared
  vocabulary.

## 9. Residual risks

1. **The respawn residual in the qual topology.** `deploy.sh` does not
   set `stord_config_path` (#385 §9.7); a stord crash mid-LVM-qual
   respawns local and every subsequent LVM open fails class validation
   — loud, but confusing. DP7 wires it; until then the risk is
   qual-only (standard installs are covered, `install.sh:1026`).
2. **`LVMBackend::open` create-on-open (DP2) changes LVM's operator
   contract.** Today provisioning is explicitly the operator's job
   (`lvm_real.rs:11-13`); after DP2 the daemon creates LVs. The VG
   must have free extents (snapshots/clones claim `100%FREE`,
   `m4.5-storage.md:217-221`) — a size-driven `lvcreate` can fail
   mid-VM-create, leaving a half-provisioned VM (the local path has
   the same create-on-open failure mode, so this is parity, not
   regression — but the failure is new to LVM operators).
3. **The locator/allowlist contract (DP5) is a security-adjacent
   decision.** Getting it wrong either denies every LVM open on
   standard installs (fail-closed, confusing) or tempts operators to
   empty the `device_allowlist` (fail-open). The recommendation keeps
   the standard install working without touching the allowlist; the
   risk is non-standard allowlists.
4. **`storage_class` NULL-vs-`"local"` duality persists.** Existing
   rows are NULL; new explicit-local rows would carry `"local"`. Every
   consumer must treat NULL and `"local"` as identical (the BFF
   already COALESCEs, C5). A missed consumer shows a spurious class
   mismatch on clones (`:888`) — the replay-match comparison uses the
   source row on both sides, so it is self-consistent, but the
   accept-time check (DP4) must normalize.
5. **The dead surfaces stay dead (by decision).** `storage_pools`,
   the inventory probe, and the fleet datastores parse remain
   misleading until Option B or deletion (§8). [Corrected 2026-10-07,
   #546:] the fleet datastores parse no longer misleads — it parses
   the enrollment string array into class-named entries with unknown
   (`Option`) capacities and suppresses `DATASTORE_NOT_FOUND` for
   offered classes (§2.5 item 2's correction); `storage_pools` and the
   inventory probe remain as stated. An operator reading the UI's
   storage page can believe pools exist that nothing serves.
6. **Seed images on LVM are unsupported (DP2 scope cut).** A VM create
   with an `image_ref` and an LVM class disk has no seed path; if PR 3
   doesn't reject that combination at accept time, it fails at
   runtime with an opaque stord error. Recommend an explicit
   accept-time rejection alongside DP4.
7. **Single-writer boundaries.** LVM open/attach rides the same
   legacy-RPC fail-closed gates in core-managed mode
   (`agent_server.rs:506-518`, `:1903-1948`); VM create rides Core.
   The split posture (create works, snapshot/clone doesn't on
   core-managed LVM nodes) is #378's recorded UX gap extended to a new
   class — disclosed, not worsened (DP8).
