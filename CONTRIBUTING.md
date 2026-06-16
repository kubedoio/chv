# Contributing to CHV

Thank you for contributing to CHV. This document covers development setup, code style, and the contribution workflow.

## Development Environment

### Required Tools

- **Rust** — latest stable via [rustup](https://rustup.rs/)
- **Node.js 20+** and npm — for the Web UI
- **protobuf-compiler** — for regenerating gRPC bindings when proto files change
- **Docker** (optional) — for containerized local deployment (`docker compose up`)

### Optional but Recommended

- `cargo-watch` — for auto-rebuilding Rust during development
- `just` or `make` — the repository includes a `Makefile` with common commands

## Building

```bash
# Rust workspace (debug)
cargo build --workspace

# Rust workspace (release)
make build

# Web UI
cd ui && npm install && npm run build

# Both
cd ui && npm run build && cd .. && cargo build --workspace --release
```

## Testing

```bash
# Rust tests
cargo test --workspace

# Rust linting
cargo clippy --workspace -- -D warnings

# Format check
cargo fmt --all -- --check

# UI build check
cd ui && npm run build

# UI type check (if available)
cd ui && npm run check
```

## Code Style

### Rust

- Format with `rustfmt` (`cargo fmt --all`)
- Lint with Clippy at `-D warnings` level (`cargo clippy --workspace -- -D warnings`)
- Prefer structured errors from `chv-errors` over panics in service code
- Use `tracing` for logging; never `println!` in library crates
- Proto contracts in `/proto` are the source of truth — do not hand-edit generated code in `/gen/rust`

### TypeScript / Svelte

- Follow the existing Prettier configuration in `ui/`
- Use TailwindCSS utility classes; avoid arbitrary values where design tokens exist
- Keep Svelte components under ~300 lines; extract helpers and sub-components when growing larger
- Use TypeScript strictly; avoid `any`

## Frontend Development Guidelines

### State Management

All mutating actions MUST use `mutateWithRefresh()` from `$lib/stores/mutation.svelte`. This ensures page cache, sidebar inventory, and task stream stay in sync.

```svelte
<!-- ✅ Correct -->
<script>
  import { mutateWithRefresh } from '$lib/stores/mutation.svelte';
  async function handleAction() {
    await mutateWithRefresh(
      () => myBffCall(args, token),
      { patterns: ['my-resource:'] }
    );
  }
</script>

<!-- ❌ Incorrect -->
<script>
  import { invalidateAll } from '$app/navigation';
  import { invalidatePattern } from '$lib/stores/api-cache.svelte.ts';
  async function handleAction() {
    await myBffCall(args, token);
    invalidatePattern('my-resource:');
    await invalidateAll();  // DON'T DO THIS
  }
</script>
```

### Compliance

CI runs `mutation-compliance.test.ts` which scans all `+page.svelte` files. Adding direct `invalidateAll` or `invalidatePattern` imports will break the build.

### New Resource Types

When adding a new resource with mutations:
1. Edit `src/lib/stores/live-state.svelte.ts` and add the task summary → cache pattern mapping to the `taskPatternMap` object inside the `LiveState` class
2. Wire the page mutation through `mutateWithRefresh()`
3. Verify your page passes the compliance tests by running `npm test -- --run src/lib/stores/mutation-compliance.test.ts`

## Changing Protocol Buffers

1. Edit the `.proto` file in `/proto/`
2. Run `cargo build --workspace` to regenerate Rust bindings
3. Update any affected TypeScript types in `ui/src/lib/types/` if the BFF contract changes
4. Update [`docs/specs/proto/`](./docs/specs/proto/) documentation if the API semantics change

## Commit Messages

Use concise, descriptive messages in the imperative mood:

```
Add quota enforcement to VM create path

- Check project quota before inserting desired state
- Return QuotaExceeded error with limit context
```

## Pull Request Workflow

1. Branch from `main` with a descriptive name: `feat/serial-console`, `fix/db-ownership`, etc.
2. Ensure CI passes: `cargo clippy`, `cargo test`, UI build
3. Update `CHANGELOG.md` under `[Unreleased]` if the change is user-facing
4. Update relevant specs or ADRs if the change affects architectural boundaries
5. Open a PR with a clear description of the problem, solution, and testing performed

## Documentation

- **Architecture decisions** → write or update an ADR in `docs/specs/adr/`
- **Component behavior** → update the component spec in `docs/specs/component/`
- **User-facing features** → update `CHANGELOG.md`
- **Deployment changes** → update `docs/DEPLOYMENT.md`
- **Design system changes** → update `DESIGN.md`

## Getting Help

- Review existing ADRs in `docs/specs/adr/` for system boundaries and invariants
- Check `docs/plans/` for the current sprint roadmap and gap analysis
- Read `CLAUDE.md` for agent-oriented build and architecture guidance

## Adding a new architecture resource kind

The Architecture Designer (see [`docs/specs/architecture-designer/`](docs/specs/architecture-designer/) and [ADR-001-Designer](docs/specs/adr/001-designer-first-class-surface.md) through [ADR-006-Designer](docs/specs/adr/006-designer-no-tosca-engine.md)) ships a closed set of CHV-native resource kinds — by design, not as a TOSCA-style open type system. Adding a new kind is an end-to-end change that touches the YAML model, schema, validation, diff, UI, and reviewer ladder.

Use this 8-step recipe. The `server` kind is a good reference: search `crates/chv-architecture-validate/src/model.rs` for `Server` to see every touchpoint.

1. **Model** — Add the kind to `crates/chv-architecture-validate/src/model.rs`. Add a struct with `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`, attach it to the `CHVArchitecture` aggregate, and update any `enum ResourceKind`-style discriminator. Round-trip through `serde_yaml` MUST work (covered by `tests/yaml_roundtrip.rs`).

2. **JSON Schema** — Update the embedded YAML schema in `crates/chv-architecture-validate/src/schema.rs`. The schema and the Rust type must drift together; the `schema_drift_test` CI gate catches mismatches.

3. **Static checks** — Add validation rules in `crates/chv-architecture-validate/src/static_checks.rs`. At minimum: name uniqueness within the kind, references resolve (e.g. a NIC's `network` points to a defined network), capacity bounds (CPU / memory / disk in the project's accepted ranges). Findings carry stable `code` strings — register the new codes in `crates/chv-architecture-validate/src/codes.rs` (the registry is CI-snapshotted; renames are blocked).

4. **Fleet check** — If the kind has a live counterpart on the running cluster (most do — networks, datastores, instances), wire it into the fleet consistency checker under `crates/chv-architecture-validate/src/fleet/`. The check compares the desired YAML against the latest `inventory_snapshot` and reports `BLOCKED_BY_FLEET` findings when prerequisites aren't satisfied.

5. **Diff rules** — Update `crates/chv-architecture-reconcile/src/diff.rs` with the create / update / delete / replace / noop rules for the kind. Field-level rules decide which mutations are in-place vs. require replacement (e.g. CPU resize is in-place; storage backend change is replace). The diff feeds the planner and shapes the user-visible plan preview.

6. **UI palette and inspector** — Add a draggable palette node under `ui/src/lib/components/architectures/palette/` with the canonical icon and label. Add a corresponding inspector pane (right-hand panel) that exposes every editable field. Keep components under ~300 lines; extract sub-components if the inspector grows. Wire the new kind into the YAML serializer in `ui/src/lib/architectures/yaml.ts` so canvas → YAML round-trips.

7. **Fixtures** — Add at least three fixtures to `crates/chv-architecture-validate/tests/fixtures/`:
   - **Positive** — a minimal valid topology that includes the new kind.
   - **Edge case** — boundary values (max name length, max resources of this kind, sparse optional fields).
   - **Negative** — a topology that violates a static check the new kind introduced; assert the expected `Finding.code`.

8. **Reviewer checklist** — On the PR, request these reviewers (per the Phase 7 review ladder):
   - `reviewer-api-contract` — confirms the schema change is backward-compatible (additive); flags any breaking field rename or type change.
   - `reviewer-security` — required if the kind is RBAC-scoped (users, roles, permissions, secrets) or affects a production-environment guarded path.
   - `reviewer-language-specialist` — Rust and TypeScript idiom and structure review.

Reviewer-test-analyzer should also confirm fixtures cover the new code paths.
