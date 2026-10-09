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
  the worker-suite level).";
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

Qualification boundary: the composed suite's always-on arm runs
against `chv-netbox-sim` (ADR-024) — a first-party stateful emulator
of the bounded wire subset the adapter exercises, whose read side
does reflect mid-phase writes; it is still not a real NetBox. The
real-NetBox qualification lane is wired (ADR-024 decision 4): the
same five scenarios run as `qualification_*` wrappers (ignored by
default) against a pinned, disposable NetBox stood up by
`deploy/netbox-qualification/docker-compose.yml` and driven by
`scripts/netbox-qualify.sh` (`make netbox-qualify`), on a weekly
schedule plus manual dispatch via
`.github/workflows/netbox-qualification.yml`. `--record` re-captures
the golden `netbox4` fixtures from the live instance — the tripwire
that turns the fixtures from authored hypotheses into recorded
evidence.

No real run has happened yet: the first dispatch happens after this
merges (GitHub workflows cannot be dispatched from a branch), and its
results will be recorded at the placeholder below. The review pass
over the lane already surfaced the expected findings. The client
write-path conformance gaps — tags sent as plain strings instead of
NetBox's nested tag objects, create ordering versus foreign-key
existence, and the missing `device_type`/`role` on device creates —
are **fixed** by the write-path conformance PR (issue #586, PR 6):
writes now use NetBox's nested reference forms (`tags` as name
dicts, `device_type` as `{"manufacturer": {"slug": "chv"}, "slug":
"chv-host"}`, `role` as `{"slug": "chv-node"}`), the kind rank is the
FK-dependency order (`vlan → prefix → device → vm → interface → ip`),
the simulator rejects device creates missing `device_type`/`role`
(mirroring NetBox 4.7's `DeviceSerializer`), and the compose init and
fixture recorder provision the `chv-node` device role alongside the
site/manufacturer/device-type/tag prerequisites. The read surface and
the `chv_` custom-field surface are unchanged, so `MAPPING_VERSION`
stays `v1`. The apply-side leg is seeded via the existing test
fixtures rather than driving the apply state machine itself.

## First real-NetBox qualification run (2026-10-09)

The qualification lane (`scripts/netbox-qualify.sh`, workflow
`netbox-qualification.yml`) ran against real NetBox 4.7.2
(netbox-docker 5.1.1) seven times before going green — every dispatch
left a merge record, and the sequence is the honest story of what the
drift tripwire is for:

1. **Run 37965573579 — false green.** The lane reported success while
   the suite had never really asserted: the `EXIT` trap reset `$?`
   (bash: `trap -` clears it), so a failed suite exited 0, and the
   NetBox healthcheck gave up long before the container was ready,
   masking boot failures. Fixed in #596 (capture `$?` before `trap -`;
   healthcheck `start_period: 300s` / `retries: 20`).
2. **Run 37967815173 — loud failure, lane bug.** The suite now failed
   loudly with logs uploaded, but before any test ran:
   `cannot create /run/qualification/netbox.env: Permission denied` —
   docker creates fresh named volumes root-owned, and the netbox user
   could not write the first file into one. Fixed in #597
   (qualification-init runs as `user: "0:0"`; a throwaway CI container
   tolerates root-owned artifacts).
3. **Run 37971829693 — the tripwire fires for real.** The lane worked
   end to end and the suite ran for true: 2 of 5 scenarios passed, 3
   failed — every failing run aborted on
   `GET /api/virtualization/interfaces/` answering 400. Root cause
   (verified against NetBox 4.7.2 sources): the interface natural-key
   probe sent `?name=…&virtual_machine=…`, and NetBox types
   `virtual_machine` as a `ModelMultipleChoiceFilter` whose form field
   validates the value **exists** — 400, not an empty page — while the
   runner probes natural keys *before* the plan creates anything, so
   the VM never exists yet. Fixed in #598 (name-only probe, VM half of
   the key applied client-side), which also added the
   `assert_run_status` diagnostics this debugging needed.
4. **Run 37976865324 — drift two.** Every projection run succeeded
   (the write path was clean), 4 of 5 scenarios green; the
   full-lifecycle state assert compared `vm["vcpus"]` against `2` and
   got `Number(2.0)` — NetBox 4.7 types `VirtualMachine.vcpus` as a
   `DecimalField`, so live rows answer `2.0` where the simulator's
   shape carries `2`. Fixed in #600 (`normalize_object` canonicalizes
   integral floats).
5. **Runs 37980195431 / 37983487636 — drift three, the deep one.**
   The re-export leg's remote-state fetch found only the IP of the six
   converged objects, planned five duplicate creates, and the
   duplicate vlan tripped NetBox's ambiguity guard on the prefix
   create (`Multiple objects match {'vid': 42}`). Dispatch 5 localized
   it with the new `assert_run_status` outcome diagnostics; #601 added
   the `probes_find_converged_state` lane test and raw-instance
   evidence capture; dispatch 6's contrast (identical queries pass in
   isolation) pinned it: **NetBox ids are unique per content type, not
   globally** — the six converged objects shared id 2, and
   `fetch_remote_state`'s `by_id` map keyed by id alone collapsed them
   into one entry, the last insert (the IP, the final probe in kind
   rank order) winning. The simulator assigns ids from one global
   counter and the wiremock mirror assigns `400 + i`, so only the real
   lane could see it. Fixed in #604 (the map and the delete-side
   ownership re-verification key by `(kind, id)`), with a wiremock
   regression test whose mirror shares one id across all six kinds —
   verified to fail against the old code with this exact symptom.
6. **Run 37987686434 — green.** All six qualification scenarios pass
   against the real instance:

   ```text
   test netbox_projection_sim_tests::qualification_foreign_object_at_natural_key_is_never_written ... ok
   test netbox_projection_sim_tests::qualification_full_lifecycle_apply_to_projection_to_reapply ... ok
   test netbox_projection_sim_tests::qualification_manual_double_enqueue_coalesces_to_one_active_run ... ok
   test netbox_projection_sim_tests::qualification_netbox_outage_never_changes_the_apply_result ... ok
   test netbox_projection_sim_tests::qualification_partial_failure_requeues_and_retry_resumes_without_duplicate_create ... ok
   test netbox_projection_sim_tests::qualification_probes_find_converged_state ... ok
   test result: ok. 6 passed; 0 failed; 0 ignored; 273 filtered out
   ```

The three genuine drifts the lane caught — the choice-filter probe
semantics, the DecimalField serialization shape, and the per-kind id
space — were all invisible to the simulator and the wiremock doubles
by construction. That is the campaign's thesis proven in production
conditions: the simulator keeps the always-on lane fast and the
qualification lane keeps the simulator honest.

Focused verification (all run, all green — the counts below are the
simulator-backend and unit-level results; the real-NetBox results are
the qualification run above):

```text
cargo test -p chv-controlplane-service netbox_projection_sim   # 6 passed (simulator backend)
cargo test -p chv-controlplane-service                          # 273 passed, 0 failed (simulator backend)
cargo test -p chv-webui-bff --test architecture_netbox_routes   # 21 passed, 0 failed
cargo clippy -p chv-controlplane-service -p chv-webui-bff --all-targets -- -D warnings  # clean
cargo fmt --all                                                 # applied
cargo check --workspace                                         # clean
cd ui && npm run check                                          # 0 errors, 0 warnings
cd ui && npm run test                                           # 374 passed (41 files)
cd ui && npx playwright test tests/e2e/architectures-netbox.spec.ts  # 5 passed
```
