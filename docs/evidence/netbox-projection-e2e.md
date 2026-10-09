# NetBox Projection E2E Evidence

Date: 2026-10-09

Scope: issue #239 integration leg, PR 8. The composed suites exercise the
projection against a bounded in-process mock NetBox and a mocked BFF wire —
the Rust suite (`crates/chv-controlplane-service/src/netbox_projection_e2e_tests.rs`)
drives topology → apply → post-apply enqueue → worker → mock NetBox, and the
UI suite (`ui/tests/e2e/architectures-netbox.spec.ts`) drives the NetBox tab
against `page.route` mocks of the BFF contract.

Machine tests prove:

- a full lifecycle converges: apply seeds the succeeded-apply run, the
  post-apply enqueue fires, and the worker creates exactly the projected
  objects in the mock NetBox with the custom-field set (`chv_external_id`,
  `chv_managed_by`, `chv_architecture_version`, `chv_mapping_version`,
  `chv_owner`) and the exact wire payloads (VM `name/status/vcpus/memory/
  device/tags/custom_fields`, device `site`), authenticated by the stored
  token;
- the dry-run plan for an already-mirrored remote is a byte-stable, fully
  `no_op`, secret-free JSON document — the plan contains no token material;
- a v2 topology (modified VM, removed server, added network) produces update
  entries for still-desired objects, marks the removed server's device stale
  (`chv_managed_state: "stale"` + `status: "decommissioning"`, never a
  delete), and creates the added objects — and re-running against the
  converged remote is idempotent (every entry `no_op`, no writes);
- every projection action lands in the audit trail with the run id, action,
  and target;
- a NetBox outage never changes the apply result: with the mock unreachable
  the apply run still finishes `Succeeded` with an unchanged `finished_at`,
  exactly one failed projection run is recorded, and later ticks do not
  duplicate runs (retry stays bounded by the existing attempt policy);
- foreign objects are never modified: a natural-key collision with a remote
  object not owned by the architecture resolves to a `conflict` entry and
  the wire log shows no write request against the foreign object id;
- the UI surfaces the whole contract: the config form renders `token_set`
  semantics (password-typed, never re-displayed) and `custom_field_prefix`,
  the dry-run table renders entries/chips/conflict cues, run history
  renders rows with retry and detail (executed-plan chips), export shows
  the 409 `NETBOX_RUN_ACTIVE` banner, and the upsert wire body carries the
  token while config/get exposes only `token_set`.

Qualification boundary: the mock NetBox implements only the bounded wire
subset the adapter exercises (the read lists, creates, and PATCHes its tests
mount); it is not a real NetBox. Real-NetBox qualification — including
version-specific custom-field provisioning and pagination behavior — is a
tracked follow-up (plan: out-of-scope list). The apply-side leg is seeded via
the existing test fixtures rather than driving the apply state machine
itself, and the mock's read side does not reflect mid-phase writes, so the
phase-2 interface fix-up PATCH is asserted on its address payload only (the
`assigned_object_id` binding is asserted in the later phase where the mirror
holds the interface).

Focused verification (all run, all green):

```text
cargo test -p chv-controlplane-service netbox_projection_e2e   # 3 passed
cargo test -p chv-controlplane-service                          # 271 passed, 0 failed
cargo clippy -p chv-controlplane-service --all-targets -- -D warnings  # clean
cargo fmt --all                                                 # applied
cd ui && npm run check                                          # 0 errors, 0 warnings
cd ui && npm run test                                           # 373 passed (41 files)
cd ui && npx playwright test tests/e2e/architectures-netbox.spec.ts  # 5 passed
```
