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
