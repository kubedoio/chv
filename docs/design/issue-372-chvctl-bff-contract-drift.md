# #372 design draft — chvctl↔BFF contract drift: the deferred subset and the missing contract test

**Issue:** kubedoio/chv#372 — the chvctl CLI and the BFF have drifted apart
on field names, routes, and response shapes. The M2.5 sweep (PR #274)
aligned part of the surface; the volume snapshot/clone pair was fixed in
the M4.5 campaign (#373). What remains: `user delete` sends a field the
BFF rejects, three command groups (`storage`, `migrate`, `backup`) target
routes that do not exist, `task watch` polls an endpoint that cannot
answer it, and `network create --vlan` is silently dropped. Root cause per
the issue: **no test drives chvctl's request bodies against the BFF, so
drift survives every refactor.**

**Status:** FINAL — investigation complete at main `422cfe8d` (verified
`git rev-parse HEAD`), 2026-10-06; all decision points adopted as
recommended (§5.1 decision record, 2026-10-06). Landing as the §6
decomposition: PR 1 (the contract-test harness, this change) → PR 2
(live-path fixes) → PR 3 (removals) → optional PR 4 (migrate reads).
Companion investigation report:
`/tmp/opencode/iss372-investigation.md` (to be attached to the issue).

**Premise corrections up front (the issue's list is partially stale):**
three of the listed deferred items are already fixed at main —

- `image import --url` — fixed by #340 (commit `12f6f39b`): chvctl sends
  `source_url` (`cmd/chvctl/src/commands/image.rs:58-63`) and the BFF reads
  it with a documented `url` back-compat alias
  (`crates/chv-webui-bff/src/handlers/images.rs:113-121`). Residual: the
  redundant dual-key send only.
- `upgrade` group — removed by #444 (commit `733fc67f`, closing #427):
  the `/v1/upgrades` routes were never registered and the backing stack
  was deleted in #213. No `Upgrade` variant remains in
  `cmd/chvctl/src/main.rs:36-97`.
- `health` group — fixed under #320: `GET /v1/health`,
  `GET /v1/cluster/health`, `GET /v1/nodes/:node_id/health` all exist
  (`crates/chv-webui-bff/src/router.rs:82-90`) and are pinned by
  `crates/chv-webui-bff/tests/health_routes.rs`.

The live remainder is four items (§2.1–2.4) plus new drift the
investigation found beyond the issue's list (§2.5–2.7), including on
surfaces that postdate the M2.5 sweep (#379's PR #511/#512).

---

## 1. Problem statement

Two coupled failures:

1. **Wire drift.** chvctl constructs request bodies inline with
   `serde_json::json!` in each command handler; the BFF reads them with
   ad-hoc `payload.get(...)` extraction. Nothing shares or pins the
   vocabulary. When either side renames a field or a route, the other side
   keeps compiling — the failure surfaces only at runtime, as a 400
   (`user delete`), a 404 (`storage`/`migrate`/`backup` groups), a silent
   field drop (`network create --vlan`), or a hang (`task watch`).
2. **No contract test.** The BFF's integration tests pin the *server* side
   of several contracts (e.g. `tests/volume_snapshot_clone.rs` pins the
   #373 fix), but no test anywhere drives *chvctl's own request-building
   path* — the exact code an operator runs — against the BFF's
   deserializer. Every fixed drift item since #274 was found by a human,
   not a test.

The question for the maintainer is threefold: (i) what the contract-test
harness looks like and where it lives; (ii) per drift item, whether the fix
is fix-client, fix-server, or remove-command; and (iii) how the
volume-create surface (#513, DP6) lands without churning the CLI↔BFF
surface a second time.

## 2. Ground truth at `422cfe8d`

### 2.1 `user delete` — 400 on every invocation (never worked)

| Side | Location | Truth |
|---|---|---|
| chvctl | `cmd/chvctl/src/commands/user.rs:58-62` | positional `username`; body `{"username": ...}` |
| BFF | `handlers/users.rs:242-296` (route `POST /v1/users/delete`, `router.rs:473-476`, admin tier) | requires `user_id` (`:249-253`); self-delete guard on `claims.sub` (:256) |
| UI | `ui/src/lib/bff/users.ts:60-66` | sends `{"user_id": ...}` — correct |
| CLI spec | `docs/specs/ops/chvctl-cli-spec.md:66` | documents `user delete <user_id>` — agrees with the BFF, not the binary |

Observed: 400 `missing user_id`. Because the command has never succeeded,
there is **no working caller to break** — its wire contract can be changed
freely. Usernames are unique (`users.rs:125-127`), so either fix direction
is unambiguous.

### 2.2 `storage` group — 404 on all four subcommands; the backing catalog is a phantom

chvctl (`cmd/chvctl/src/commands/storage.rs`) vs the BFF router:

| Subcommand | chvctl sends | file:line | BFF truth | Observed |
|---|---|---|---|---|
| `list` | `GET /v1/storage/pools` | `:41` | `POST /v1/storage-pools` (`router.rs:154-157`) | 404 (path + method) |
| `show` | `GET /v1/storage/pools/{id}` | `:54-56` | no per-pool get route exists | 404 |
| `create` | `POST /v1/storage/pools` `{name, backend, path}` | `:64-65` | `POST /v1/storage-pools/create` `{name, node_id, pool_type\|backend_class, path, capacity_bytes\|total_bytes}` (`router.rs:305-308`, `handlers/storage.rs:63-130`) | 404; `backend` is read by nothing (`handlers/storage.rs:80-85`) |
| `delete` | `DELETE /v1/storage/pools/{id}` | `:70-72` | no delete route exists | 404 |

Capability truth: the `storage_pools` table
(`cmd/chv-controlplane/migrations/0015_storage_pools.sql`) is a UI-facing
catalog that **nothing in provisioning reads or writes** — recorded as
phantom surface C4 in the #379 design doc
(`docs/design/issue-379-storage-class-dispatch.md:117`) and left dead by
decision (residual 5, `:712-715`). The UI calls list/create
(`ui/src/lib/bff/endpoints.ts`); the CP stub mirrors it
(`crates/chv-controlplane-service/src/api/router.rs:128-131`). chvctl's
`--backend` doc (`storage.rs:20-22`) also predates the DP3 vocabulary
(missing `lvm`; #379's C2).

### 2.3 `migrate` group — 404 on all four subcommands; the capability is real but differently shaped

| Subcommand | chvctl sends | file:line | What exists | Observed |
|---|---|---|---|---|
| `start` | `POST /v1/migrations` `{vm_id, target_node}` | `migrate.rs:37-38` | `POST /v1/vms/mutate` `{vm_id, action:"migrate", target_node_id}` (`handlers/vms.rs:1206-1221`) — what `chvctl vm migrate` already does (`vm.rs:149-156`) | 404 |
| `status` | `GET /v1/migrations/{id}` | `:43-45` | nothing | 404 |
| `cancel` | `POST /v1/migrations/{id}/cancel` | `:48-53` | `POST /admin/migrations/{id}/cancel` — CP admin router (`api/router.rs:90-93`, `api/migrations.rs:26-79`), admin-tier, no body | 404 |
| `list` | `GET /v1/migrations` | `:56` | nothing | 404 |

Capability truth: the migration machinery is real and live — `migrations`
table (`migrations/0038_migration_operations.sql`), CP migration loop with
cooperative cancel (`crates/chv-controlplane-service/src/migration.rs:344,428-438`).
Only BFF-tier entry points and HTTP read surfaces are missing. chvctl also
uses `target_node` where the vm-mutate contract wants `target_node_id`.

### 2.4 `backup` group — 404 on both subcommands; "execution" is a catalog insert

| Subcommand | chvctl sends | file:line | What exists | Observed |
|---|---|---|---|---|
| `list` | `GET /v1/backups` | `backup.rs:28` | `GET /v1/backups/jobs|schedules|restores` (viewer tier, `router.rs:199-223`); legacy `GET /api/v1/backup-jobs` (`api/router.rs:55-83`) | 404 |
| `run` | `POST /v1/backups/run` `{vm_id, label?}` | `:48-52` | `POST /v1/backups/jobs/:job_id/execute` (`router.rs:363-366`) | 404 |

Capability truth (regrounded per verification review): **a live
`BackupWorker` scaffold exists** (`crates/chv-controlplane-service/src/
backup_worker.rs`, spawned at `cmd/chv-controlplane/src/bootstrap.rs:
424-431`) — cron scheduler, atomic job claims, exponential-backoff
retries, NFS/S3 retention shippers — **but its `execute_job` (`:452-468`)
is a guaranteed-fail no-op** ("Backup execution engine not yet
implemented"): every claimed job is actively Failed within ~30 s, and the
production-readiness declaration agrees (`00-execution-declaration.md:61`:
"backup_worker execute is guaranteed-fail no-op; restore = DB record
only. **Backup is not DR.**"). Separately there is zero backup machinery
in the agent or the node/control-plane protos (no `backup` match in
`crates/chv-agent-core/src`, `cmd/chv-agent/src`, `proto/node/*`,
`proto/controlplane/*`), and the BFF's "execute" route only inserts
another job row (`handlers/backups.rs:394-438`, `:960-1004`). `label` is
read by nothing. So a repointed `backup run` would not report success —
it would enqueue a job that a live process then Fails; the CLI surface
would be a broken capability with real state behind it, not a phantom.

### 2.5 `task watch` — polls a list endpoint that cannot answer; hangs forever

> **Superseded (2026-10-06, PR 2 / #519):** the findings in this
> section describe the pre-fix command and are retained as the
> historical record. `task watch` now polls the new
> `POST /v1/tasks/get` (the house `/get` convention), matches the real
> capitalized status vocabulary, and is bounded by `--timeout`
> (default 15 min) — see DP6 below and the §5.1 decision record.

chvctl (`cmd/chvctl/src/commands/task.rs:37-59`) polls `POST /v1/tasks`
with `{"task_id": ...}` (:41), reads a top-level `status` (:43-46), exits
on `"completed"|"failed"|"cancelled"` (:50-51). The handler
(`handlers/tasks.rs:13-145`) reads only `page`/`page_size`/
`filters{status,resource_kind,window}` (:21-35) — `task_id` is silently
ignored — and the response is `{items, page, filters}` (:133-144) with no
top-level `status`. Result: `Status: unknown` every 2s, forever. Even with
a correct endpoint the vocabulary is wrong: operation statuses are
`Pending, Accepted, Running, RetryPending, Succeeded, Failed, Rejected,
Cancelled, Stale, Conflict, AwaitingOperatorInput`
(`crates/chv-controlplane-types/src/domain.rs:293-305`) — capitalized
`Succeeded`, never `completed`. No single-task get route exists; the UI
also only lists (`ui/src/lib/bff/tasks.ts`).

### 2.6 `network create --vlan` — silently dropped; the capability does not exist

chvctl adds `"vlan"` when `--vlan` is passed
(`cmd/chvctl/src/commands/network.rs:20,48-53`). The BFF's `create_network`
(`handlers/networks.rs:223-298`) reads `name, cidr, gateway, bridge_name,
dhcp_enabled, ipam_mode, is_default, nat_rules, dhcp_scope, dns_enabled,
dns_scope, firewall_rules` — no `vlan`; dropped with no error. The
capability is absent end-to-end: the `networks` table has no `vlan_id`
column (`crates/chv-controlplane-store/src/networks.rs:27-28` — "Always
`None` until the schema gains a vlan_id column") and the UI's
`CreateNetworkInput` has no vlan field (`ui/src/lib/bff/types.ts:629-637`).

### 2.7 New drift found beyond #372's list (the M2.5 sweep is stale)

Full-surface census of all 43 chvctl request-building paths; everything
unlisted matches (§6 records the clean set).

**(a) Display-column drift — silent empty columns on four `list`
commands:**

| Command | chvctl columns | BFF item fields | Phantom columns |
|---|---|---|---|
| `task list` | `task_id, type, status, resource_id, created_at` (`task.rs:31-35`) | `task_id, status, operation, resource_kind, resource_id, actor, started_unix_ms, finished_unix_ms` (`handlers/tasks.rs:117-131`) | `type`, `created_at` |
| `network list` | `network_id, name, cidr, vlan, status` (`network.rs:42-46`) | `network_id, name, scope, health, attached_vms, exposure, policy, last_task, alerts, dhcp_enabled, ipam_mode, is_default` (`handlers/networks.rs:80-98`) | `cidr`, `vlan`, `status` |
| `volume list` | `volume_id, name, size, status, attached_to` (`volume.rs:42-46`) | `volume_id, name, node_id, health, size, attached_vm_id, attached_vm_name, status, last_task` (`handlers/volumes.rs:71-86`) | `attached_to` |
| `image list` | `image_id, name, format, size, status` (`image.rs:42-46`) | `image_id, name, size, status, os, version, usage_count, last_updated` (`handlers/images.rs:57-71`) | `format` |

**(b) CLI capability gap on the #379 surface (new since PR #511/#512):**
the BFF VM-create handler accepts `node_id` (`handlers/vms.rs:339-351`),
`volume_size_gb` (:466-480), `cloud_init_userdata` (:482-486), and
`storage_class` (:488-515, validated against the DP3 vocabulary home
`chv_hypervisor_api::resources`, unknown → 400, accept-time
`ensure_node_offers_storage_class` at `lifecycle.rs:417,693`).
`chvctl vm create` sends only `name, cpu_count, memory_bytes, image_ref,
network_id` (`vm.rs:110-123`) — **the CLI cannot express the storage class
that #379 just enabled.** Client-behind-server drift, exactly #372's
failure class, on a surface that did not exist at M2.5.

**(c) The CLI spec is a third drifted voice**
(`docs/specs/ops/chvctl-cli-spec.md`): documents nonexistent commands
(`volume show/create/delete` :35-37, `image show` :43, `network show` :49,
`task show` :55, `backup show/create/restore` :59-61), spells the storage
subcommands differently than the binary (:38-39), documents
`user delete <user_id>` (:66 — disagrees with the binary), and cites the
wrong credentials path (:84 `~/.chv/credentials` vs `config.rs:10-22`
`~/.config/chvctl/`). Any fix must align this spec in the same PR.

### 2.8 How chvctl requests are built, and what a harness can drive

Bodies are built inline in command handlers (`cmd/chvctl/src/commands/*.rs`)
on a thin reqwest wrapper (`cmd/chvctl/src/client.rs:28-109`; Bearer at
:51-57; `.json()` sets the `Content-Type: application/json` the CSRF
middleware requires, `csrf_middleware.rs:4-23`). chvctl is a **bin-only
crate** (`Cargo.toml:9-11`) — the single structural obstacle; §4 removes
it. The BFF test suite already has the full server-side pattern:
in-memory SQLite + `run_migrations` + hand-built `AppState` +
`NoopMutations`/`RecordingMutations` + seeded JWT + `bff_router(state)`
(`tests/volume_snapshot_clone.rs:165-264`, same shape in 21 test files).
chvctl's default URL matches the CP bind (`main.rs:108` ↔
`chv-config/src/lib.rs:657`).

## 3. Goals & non-goals

**Goals**

- Every chvctl request-building path is pinned by a test that drives the
  CLI's real client code against the real BFF router — the drift class
  dies, not just this instance.
- Every live 400/404/hang/silent-drop in §2 is fixed or removed.
- The CLI can reach the #379 storage-class surface (`vm create
  --storage-class` and friends).
- The volume-create contract names are frozen once (§5 DP8) so #513 lands
  without a second reconciliation pass.

**Non-goals** (§8 restates with reopen triggers): implementing VLANs;
implementing a backup executor; making the `storage_pools` catalog
truthful (#379 Option B); implementing the volume-create API itself (#513);
rolling upgrade (removed, #444); any change to the legacy A10 CreateVm RPC
branch beyond what the contract test observes from the BFF side.

## 4. Options — the contract-test harness

### Option A — table-driven golden-body unit tests

Extract pure body-builder functions per command and assert the JSON they
emit against golden strings. *Rejected:* pins only half the contract (the
BFF's side can still drift — a renamed route or a handler that starts
requiring a new field passes); golden JSON rots into noise; doesn't cover
method/path/auth-layer mistakes, which is where three of the four live
drift items actually live (404s, not field renames).

### Option B — drive chvctl's real client against the real BFF router over TCP (recommended)

chvctl gains a lib target (`src/lib.rs` re-exporting
`client`/`commands`/`config`/`output`; `main.rs` consumes it — mechanical,
no behavior change). A new `cmd/chvctl/tests/contract.rs`:

1. builds `AppState` exactly like the BFF tests do (in-memory SQLite,
   `run_migrations`, `NoopMutations`/`RecordingMutations`, test JWT
   secret);
2. binds a real ephemeral listener (`TcpListener::bind("127.0.0.1:0")`) and
   serves **`admin_router(state)`** with `axum::serve` — the CP admin
   router merges `bff_router` and additionally mounts the admin-tier
   routes (`/admin/migrations/{id}/cancel`, `api/router.rs:90-93`) that
   the repointed `migrate cancel` row needs; it reuses the same
   `AppState` and `admin_middleware` accepts the seeded BFF JWT.
   Constructing it requires `SharedConvergenceMetrics` (check
   `api/router.rs`'s signature — a default/empty instance suffices for
   these rows). A `bff_router`-only listener would 404 the migrate-cancel
   row even post-fix;
3. constructs `BffClient::new("http://127.0.0.1:{port}", Some(token))` and
   invokes `commands::<group>::execute(&client, args, &format)` — the
   operator's actual code path, including method, path, headers, body;
4. asserts per row: status ∉ {404, 405} (route + method exist); not a
   `missing <field>` 400 (field names accepted); `RecordingMutations`
   received the expected method:args; every column key chvctl prints
   exists in a response item (kills the §2.7(a) display-drift class).

Dev-dependencies (no cycle; the BFF does not depend on chvctl):
`chv-webui-bff`, `chv-controlplane-store`, `chv-common`, `sqlx`, `axum`,
`tempfile`. Obstacles and their resolution: auth middleware → seeded JWTs
(`seed_jwt_as` pattern, role per row); CP-dependency in handlers → the
Noop/Recording `MutationService` already decouples it; CSRF → reqwest's
`.json()` sets the required content type; `login` writing the real config
dir → `config.rs` honors `XDG_CONFIG_HOME`, point it at a tempdir;
login's in-process rate limiter (`handlers/auth.rs:51-88`) → a
non-obstacle for a single seeded-token row, but if `login` itself ever
becomes a harness row, reset or bypass the map per test;
`task watch`'s pre-fix hang → wrap in `tokio::time::timeout`, assert the
timeout plus at least one poll hit (flips to a completion assertion in the
fix PR). One footnote for the removal decision: the dead groups'
response-parse keys are *also* wrong against the real handlers'
(`storage list` reads `.get("pools")`, `migrate list` `.get("migrations")`,
`backup list` `.get("backups")`) — irrelevant if the groups are removed,
but any "repoint" option must fix the read shape too, not just the route.

Cost: the lib split plus ~30 live rows today; each subsequent fix adds its
row. This is the harness the issue's root-cause statement asks for — it
would have caught every item in §2 at PR time.

### Option C — a shared request-types crate (chvctl + BFF both link it)

A `chv-bff-contract` crate holding typed request/response structs both
sides use. *Deferred:* the right long-term shape if the CLI grows, but it
touches every BFF handler (ad-hoc `payload.get` → typed extract) — a
rewrite of the surface, not a reconciliation, and it would collide with
#513's additions. Revisit as a follow-up once the harness has stabilized
the vocabulary (§8).

## 5. Decision points (maintainer)

### 5.1 Decision record — ADOPTED 2026-10-06

All decision points below were adopted as recommended by the maintainer
on 2026-10-06: DP1 Option B (real chvctl client over TCP, tests in
`cmd/chvctl/tests/contract.rs` behind a chvctl lib target); DP2
`user delete` → `user_id`; DP3 `storage` group → remove; DP4 `migrate`
→ repoint `start` (via the vm-mutate path, `target_node_id` field fix)
and `cancel` (admin tier, disclosed), drop `list`/`status`, with DP4b's
viewer-tier read routes as the optional PR 4; DP5 `backup` group →
remove (regrounded basis: the live `BackupWorker` scaffold's execute is
a guaranteed-fail no-op — every enqueued job is actively Failed within
~30 s; the worker itself is untouched by the removal); DP6 `task watch`
→ new `POST /v1/tasks/get` + status-vocabulary fix + `--timeout` (default
15 min); DP7 `network create --vlan` → remove the flag, file the VLAN
follow-up issue; DP8 the #513 freeze list as drafted with
`attached_vm_id` reserved; DP9 all four `vm create` flags (`--node`,
`--storage-class`, `--disk-size-gb`, `--cloud-init`) in the fixes PR;
DP10 display-column corrections + dual-key send cleanup + cli-spec
alignment; DP11 removals as their own approval-gated PR (the #444
precedent). Campaign order: PR 1 harness → PR 2 live-path fixes →
PR 3 removals → optional PR 4 migrate reads; #513's volume-create PR is
a separate campaign on the DP8 freeze.

**DP1 — harness placement and shape.** *Recommendation: Option B, tests in
`cmd/chvctl/tests/contract.rs` behind a chvctl lib target.* Alternative:
`crates/chv-webui-bff/tests/chvctl_contract.rs` with a path dev-dependency
on chvctl (keeps all contract tests in the BFF suite; equally valid — pick
by review ownership preference). Option A alone is insufficient; Option C
is the destination, not the first step.

**DP2 — `user delete` fix direction.** *Recommendation: fix-client, the
positional arg becomes `user_id`* — matches the UI and the already-written
spec, minimal code; add `user_id` to `user list`'s columns so ids are
discoverable. The command has never worked, so there is no compat surface.
*Alternative:* keep the username UX and resolve `username → user_id`
client-side via `POST /v1/users` (unique, `users.rs:125-127`) before the
delete — friendlier, one extra round trip. Fix-server (accept `username`)
is rejected: it diverges from the UI's `user_id` contract and adds an
alias the BFF must carry forever.

**DP3 — `storage` group.** *Recommendation: remove the group* — the
catalog is a phantom surface (#379 C4/residual 5) and #444 is the exact
precedent (removal + docs alignment + the alternative explicitly
declined). Repointing would require two new BFF routes (per-pool get,
delete), a `--backend`→`--pool-type` rename, and would lend CLI
credibility to a catalog nothing provisions from. The catalog's fate
belongs to #379 Option B, not to chvctl.

**DP4 — `migrate` group.** *Recommendation (minimal truth): repoint
`migrate start` to delegate to the vm-mutate path (what `vm migrate`
already does — `target_node_id`, `migrate.rs:37` field fix either way);
repoint `migrate cancel` to `POST /admin/migrations/{id}/cancel` (admin
token required — disclose in docs); remove `migrate status`/`list` unless
DP4b is taken.* **DP4b (optional PR):** add viewer-tier `GET /v1/migrations`
and `GET /v1/migrations/{id}` as plain SELECTs over the existing
`migrations` table — small, genuinely useful for operators, and the
machinery is real. *Alternatives:* full removal (loses the cancel entry
point); full implementation including BFF-tier start (redundant with
`vm migrate`).

**DP5 — `backup` group.** *Recommendation: remove the group.* §2.4
(regrounded): a live `BackupWorker` scaffold exists, but `execute_job`
is a guaranteed-fail no-op — every enqueued job is actively Failed by
the worker within ~30 s, and the production-readiness declaration says
plainly "Backup is not DR." A repointed `backup run` would not lie with
a success; it would hand the operator a surface whose every invocation
ends in a worker-Failed job — still worse than not exposing it, and it
would imply the capability is supported. *Alternative:* keep a read-only
`backup list` repointed to `GET /v1/backups/jobs` (+ `/schedules`) —
better grounded than first drafted (a real schedule/job/retention state
machine exists behind the catalog, and the UI keeps its pages), so this
is a genuine option if operators want CLI visibility — but `backup run`
must still go. Any future backup-executor design must build on (or
replace) the existing worker scaffold, not start from zero.

**DP6 — `task watch`.** *Recommendation: fix-server + fix-client.* Add
`POST /v1/tasks/get` `{task_id}` (the house `/get` convention:
`vms/get`, `nodes/get`, `volumes/get`, `networks/get`) returning the
single operation row; fix chvctl to poll it, match the real status
vocabulary (`Succeeded`/`Failed`/`Cancelled`, capitalized), and add a
poll cap (--timeout, default e.g. 15 min) so a stuck task can't hang the
CLI. The route also serves a future UI task-detail page. *Alternative:*
remove `watch` — rejected: it's the natural operator loop
(create → watch) and the fix is small.

**DP7 — `network create --vlan`.** *Recommendation: remove the flag and
the `vlan` list column* — the capability does not exist at any layer
(§2.6); a flag that silently does nothing is the exact failure mode #372
names. File a follow-up issue for VLAN support (schema + BFF + nwd) if
wanted. *Alternative:* reject loudly (`--vlan is not supported by the
server`) — acceptable if the maintainer prefers keeping the flag
discoverable; implementing it is out of scope (feature campaign).

**DP8 — #513 co-design interface (volume-create contract freeze).** Per
#379's DP6 decision (`issue-379-storage-class-dispatch.md:508-516,
:570-571`), the volume-create API is a separate issue (#513) designed
jointly with this pass. *Recommendation: this pass freezes the names and
reserves the harness slot; #513's PR implements.* Freeze list, mirroring
the vm-create vocabulary (`handlers/vms.rs:322-515`):

- route `POST /v1/volumes/create` (operator tier, beside
  `volumes/mutate|snapshot|clone`, `router.rs:285-304`);
- `name` + `display_name` alias, validated by `is_valid_display_name`;
- `node_id` (required — standalone volumes have no VM to place them);
- `capacity_bytes` (i64 — the store's native unit, consistent with
  `memory_bytes`; chvctl converts `--size 10G` via the existing
  `parse_size_bytes`, `vm.rs:181-213`);
- `storage_class` — optional, validated against
  `chv_hypervisor_api::resources` exactly like the vm-create arm
  (`handlers/vms.rs:497-515`); absent = NULL = local (DP3);
- `seed_image_ref` — deferred; #513 must reject the combination with a
  non-local class at accept time (LVM has no seed path, #379 DP2 scope
  cut; same shape as residual 6);
- `attached_vm_id` — **reserved, not frozen**: #513's open question 2
  (attachable-at-create) is unanswered; if #513 answers yes the frozen
  surface grows additively (a new optional key), never by renaming —
  renames are what the freeze forbids;
- accept-time checks: reuse `ensure_node_offers_storage_class`
  (`lifecycle.rs:417`) and the #495 `ensure_*` discipline; #513 must
  answer the new question of standalone volume create on a core-managed
  node (#378 posture);
- agent side: `CreateVolume` RPC mirroring `CreateVmRequest`
  (`proto/controlplane/control-plane-node.proto:196-200`) per #379 DP6 —
  not this pass;
- harness: #513's PR adds the `volume create` row (body → route →
  deserializer → journaled mutation); this pass adds only the reserved
  slot and the freeze note.

**DP9 — `vm create` capability flags (§2.7b).** *Recommendation: include
in the client-fix PR* — `--node` (`node_id`), `--storage-class`
(`storage_class`, choices from the DP3 vocabulary), `--disk-size-gb`
(`volume_size_gb`), `--cloud-init` (`cloud_init_userdata`). Without
`--storage-class` the CLI cannot reach the #379 LVM enablement that
landed four days ago; the surface would stay UI-only. Client-only change,
server already validates (unknown class → 400 at accept, `lifecycle.rs:693`).

**DP10 — image-import dual-key cleanup.** *Recommendation: drop the
redundant `url` key from chvctl's body* (`image.rs:58-63`); the server
alias (`handlers/images.rs:117-119`) stays for old builds. Trivial; fold
into the client-fix PR.

**DP11 — dead-group removal as its own PR.** *Recommendation: yes* —
removal is user-visible and warrants its own approval, exactly as #444
did (removal PR separate from fixes; docs aligned in the same PR:
OPERATIONS, ARCHITECTURE, DEPLOYMENT-ARCHITECTURE, cli-spec).

## 6. Test strategy

Existing pins to keep green: the full `chv-webui-bff` integration suite —
in particular `tests/volume_snapshot_clone.rs` (#373's contract pins, the
template for the harness), `tests/health_routes.rs` (#320),
`tests/vm_create_storage_class.rs` (#379 PR 2 — the accept-time 400s the
new `--storage-class` flag rides), `tests/network_delete.rs`,
`tests/router_role_gates.rs`; `cmd/chvctl`'s unit tests
(`parse_size_bytes`, credentials round-trip).

New per PR:

- **PR 1 (harness):** `cmd/chvctl/tests/contract.rs` — a row per live
  command (~30), each asserting route existence, field acceptance,
  mutation-service forwarding, and display-column presence; rows for
  `user delete` (asserts today's 400, flipped in PR 2) and `task watch`
  (timeout + poll-hit assertion, flipped in PR 2) recorded as red.
- **PR 2 (live fixes):** flip the red rows green; new BFF test rows for
  `POST /v1/tasks/get` (found/not-found/shape); harness rows for the new
  `vm create` flags (incl. `--storage-class lvm` forwarded and
  `--storage-class bogus` → 400); display-column rows now assert the
  corrected columns.
- **PR 3 (removals):** harness loses the removed groups' rows;
  `chvctl --help` snapshot in docs; cli-spec aligned.
- **PR 4 (optional migrate reads):** BFF tests for the two new GET routes
  (empty table, seeded migration row, not-found); harness rows for
  repointed `migrate start`/`cancel`.
- **#513's PR:** harness row for `volume create` per the DP8 freeze;
  store/BFF/RPC/agent tests per its own design.

Validation ladder: `cargo test -p chvctl`, `cargo test -p chv-webui-bff`,
then `cargo test --workspace` + clippy/fmt before landing each PR.

## 7. Rollout & rollback

- **PR 1** is test-only plus the bin→lib split — no behavior change;
  rollback is revert.
- **PR 2** changes CLI flags (`user delete <user_id>`, `--vlan` removal,
  new `vm create` flags) and adds one read-only BFF route
  (`POST /v1/tasks/get`, viewer tier). The flag removals break only
  invocations that already failed (400) or silently did nothing — zero
  working usage exists to break. The new route is additive. `task watch`
  gains a timeout — behavior change disclosed in the changelog.
- **PR 3** removes user-visible command groups that can only 404 today.
  Rollback is revert; the #444 playbook (docs recorded in past tense,
  alternative explicitly declined, follow-up issue referenced) applies
  verbatim.
- **PR 4** adds two read-only routes and repoints two subcommands; the
  cancel repoint crosses into admin-tier auth — document the role
  requirement in the cli-spec.
- **#513** carries its own rollout (journaled mutation through
  `bff_mutations.rs`, whose `VolumeLookupRow` already selects
  `storage_class` across five arms — `:28-34`, arms at `:406, :497, :545,
  :593, :641` — so the create path joins an established pattern).

## 8. Non-goals / scope boundaries (with reopen triggers)

- **VLAN support** (schema, BFF field, nwd enforcement) — *reopen when a
  deployment need is recorded; follow-up issue per DP7.*
- **Backup execution** — *reopen when a backup executor design exists;
  until then no CLI verb may imply one (DP5).*
- **Making `storage_pools` truthful / Option B** — belongs to #379's
  deferred Option B; *reopen per its triggers.*
- **Volume-create implementation** — #513's PR per DP8; this pass freezes
  names only.
- **Typed shared contract crate (Option C)** — *reopen when the harness
  has stabilized and the surface stops moving (post-#513).*
- **Legacy A10 CreateVm RPC branch** — dead-but-live surface recorded by
  #379; untouched here (the harness drives the BFF, not the agent RPC).

## 9. Residual risks

1. **The harness pins chvctl↔BFF, not BFF↔lifecycle.** The CP's
   `bff_mutations.rs` journal layer can still drift from the BFF handler
   expectations; `RecordingMutations` rows catch the interface but not
   the ControlPlaneMutationService implementation. The existing BFF/CP
   test suites remain the guard there.
2. **Removals strand muscle memory.** Operators with `storage`/`backup`/
   `migrate` in scripts get "unrecognized subcommand" instead of 404 —
   louder and truthful, but a change; the changelog and #444-style docs
   must call it out.
3. **`migrate cancel`'s admin tier** may surprise operators holding
   operator-role tokens (403). Disclose in the cli-spec; DP4b's read
   routes stay viewer-tier. The route also lives on the CP service's
   `admin_router` path, not the plain BFF router — the harness serves
   `admin_router` for exactly this reason (§4 step 2), and operators
   must point a CP admin endpoint at it (deployment note for the
   cli-spec).
4. **`task watch` timeout default** is a UX judgment (recommend 15 min);
  too low aborts legitimate long migrations, too high recreates the hang.
  Make it a flag.
5. **The freeze list (DP8) is a recommendation until #513's PR reviews
  it** — if #513's design changes a name, the harness row and this doc
  must move together; the harness is the enforcement, the doc is the
  intent.
6. **Display columns can drift again server-side** (a handler dropping a
  response field empties a column silently) — the harness's
  column-presence assertion covers the fields chvctl prints, which is the
  operable subset; full response-shape pinning stays with the BFF tests.
