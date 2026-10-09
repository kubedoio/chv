# NetBox Projection E2E Evidence

Date: 2026-10-09

Scope: issue #239 integration leg, PR 8. The composed suites exercise the
projection against a bounded in-process NetBox-shaped backend and a mocked
BFF wire — the Rust suite
(`crates/chv-controlplane-service/src/netbox_projection_sim_tests.rs`,
since issue #586's PR 3 a stateful `chv-netbox-sim` instance with
state-dump assertions; originally delivered against a wiremock double)
drives topology → apply → post-apply enqueue → worker → simulated NetBox,
and the UI suite (`ui/tests/e2e/architectures-netbox.spec.ts`) drives the
NetBox tab against `page.route` mocks of the BFF contract.

Machine tests prove:

- a full lifecycle converges: apply seeds the succeeded-apply run, the
  post-apply enqueue fires, and the worker creates exactly the projected
  objects in the simulated NetBox with the custom-field set (`chv_external_id`,
  `chv_managed_by`, `chv_architecture_version`, `chv_mapping_version`,
  `chv_owner`) and the exact wire payloads (VM `name/status/vcpus/memory/
  device/custom_fields`, device `site`), authenticated by the stored
  token;
- the dry-run plan for an already-mirrored remote is a fully `no_op`,
  secret-free JSON document — the recorded outcome (envelope + executed
  plan, what the BFF serves) contains no token material; the plan's
  byte-stability across re-computations is proven at the adapter unit
  level;
- a v2 topology (modified VM, removed server, added network) produces update
  entries for still-desired objects, marks the removed server's device stale
  (`chv_managed_state: "stale"` + `status: "decommissioning"`, never a
  delete), and creates the added objects. The literal no-op assertions here
  are (a) the phase-3 dry-run over the v1 mirror is fully `no_op`, and
  (b) phase 5's no-new-apply tick produces no further writes; the v2
  re-export's full-no-op equivalence is proven at the worker-suite level
  (`export_creates_all_objects_then_re_run_is_all_no_op`), not re-asserted
  in this suite;
- a partial failure mid-plan auto-requeues the run with a backoff, and
  the resumed attempt resolves the half-created object by its natural
  key: the backend state after the retry has exactly one object per
  natural key (no duplicate create — asserted on the state dump, not
  the wire), the remaining kinds complete, and the final remote state
  is converged (dry-run fully `no_op`). (Against the NetBox-shaped
  simulator a mid-plan create failure cannot be faulted per HTTP
  method, so the half-created state is seeded via the control plane;
  the mid-plan-create + partial-ledger composition itself is proven at
  the worker-suite level.);
- a second manual export enqueue while one is queued/running answers the
  one-active conflict (`is_active_run_conflict`, the same classification
  the BFF maps onto 409 `NETBOX_RUN_ACTIVE`), and the post-apply sweep
  coalesces onto the single active run — exactly one run row, executed
  once;
- every projection action lands in the audit trail with the run id, action,
  and target;
- a NetBox outage never changes the apply result: with the simulated NetBox
  failing every request (injected 5xx fault) the apply run still finishes
  `Succeeded` with an unchanged `finished_at`,
  exactly one failed projection run is recorded, and later ticks do not
  duplicate runs (retry stays bounded by the existing attempt policy);
- foreign objects are never modified: a natural-key collision with a remote
  object not owned by the architecture resolves to a `conflict` entry and
  the foreign object is byte-identical in the backend state dump afterward
  (no ownership custom fields added, `last_updated` untouched);
- the UI surfaces the whole contract: the config form renders `token_set`
  semantics (password-typed, never re-displayed) and `custom_field_prefix`,
  the dry-run table renders entries/chips/conflict cues, run history
  renders rows with retry and detail (per-entry outcomes and executed-plan
  chips from the flat `result_json` the BFF serves after unwrapping the
  worker's provenance envelope — `plan_json` is null on the real wire),
  export shows the 409 `NETBOX_RUN_ACTIVE` banner, and the upsert wire body
  carries the token while config/get exposes only `token_set`.

Qualification boundary: the composed suite runs against `chv-netbox-sim`
(ADR-024) — a first-party stateful emulator of the bounded wire subset the
adapter exercises, whose read side does reflect mid-phase writes; it is
still not a real NetBox. Real-NetBox qualification — including
version-specific custom-field provisioning and pagination behavior — is a
tracked follow-up (plan: out-of-scope list; delivery tracked in #586's
PR 5). The apply-side leg is seeded via
the existing test fixtures rather than driving the apply state machine
itself.

Focused verification (all run, all green):

```text
cargo test -p chv-controlplane-service netbox_projection_sim   # 5 passed
cargo test -p chv-controlplane-service                          # 274 passed, 0 failed
cargo test -p chv-webui-bff --test architecture_netbox_routes   # 21 passed, 0 failed
cargo clippy -p chv-controlplane-service -p chv-webui-bff --all-targets -- -D warnings  # clean
cargo fmt --all                                                 # applied
cargo check --workspace                                         # clean
cd ui && npm run check                                          # 0 errors, 0 warnings
cd ui && npm run test                                           # 374 passed (41 files)
cd ui && npx playwright test tests/e2e/architectures-netbox.spec.ts  # 5 passed
```
