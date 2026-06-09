# CHV - Cloud Hypervisor Virtualization Platform

## Project Overview

CHV is a Rust-first virtualization management repository with a SvelteKit frontend and proto/spec-driven backend direction. It provides API-driven VM lifecycle management built on Cloud Hypervisor for sovereign private cloud and edge environments.

## Repository Direction

- Active backend/control-plane language: **Rust**
- Active backend workspace: `/Cargo.toml`, `/cmd`, `/crates`, `/gen/rust`
- Authoritative contracts: `/proto`
- Authoritative design and behavior docs: `/docs/specs`, `/docs/plans`
- Current phase: Early-to-MVP transitioning to stability (see [`PHASED_IMPLEMENTATION_PLAN.md`](PHASED_IMPLEMENTATION_PLAN.md))

## Build Commands

```bash
# Rust workspace
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all

# Frontend
cd ui && npm install && npm run build

# Release packaging
make build-release        # Build binaries + tarball
make package-deb          # Build .deb packages (requires nfpm)
make package-rpm          # Build .rpm packages (requires nfpm)
make package-local        # Build both formats
make package-smoke-deb    # Smoke test .deb in Docker
make package-smoke-rpm    # Smoke test .rpm in Docker
make check-release-local  # Run all local release checks

# Local dev install with systemd units
make dev-install
```

## Release Engineering

**If you are working on releases, packaging, CI/CD, or versioning, read [`docs/release/PIPELINE.md`](docs/release/PIPELINE.md) first.**

Quick reference:
- Version source of truth: [`VERSION`](VERSION)
- Version derivation: [`scripts/version.sh`](scripts/version.sh)
- Package builder: [`scripts/build-packages.sh`](scripts/build-packages.sh)
- CI workflows: [`.github/workflows/`](.github/workflows/)
- Release process: [`docs/release/release-process.md`](docs/release/release-process.md)

## Proto Generation

If you change `.proto` files:

```bash
cargo build --workspace
```

The workspace `build.rs` files use `tonic-build` to regenerate code in `/gen/rust`. Do not hand-edit generated files.

## Backend Implementation Rules

- New backend/control-plane work belongs in the Rust workspace, not any archived Go tree.
- Proto contracts in `/proto` are the source of truth for inter-service APIs.
- ADRs and component specs in `/docs/specs` define the intended system boundaries.
- Use `chv-errors` for structured errors; avoid panics in service code.
- Use `tracing` for logging; never `println!` in library crates.
- Keep Svelte components under ~300 lines; extract helpers when growing larger.
- Use `mutateWithRefresh()` for all WebUI mutations; never call `invalidateAll()` or `invalidatePattern()` directly in page components

## Key Files for Context

| File | Why it matters |
|------|---------------|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | High-level architecture, data flow, current phase |
| [`docs/specs/adr/`](docs/specs/adr/) | Boundaries and invariants (agent/stord/nwd split, control-plane boundary, state machines, error handling, logging, async safety) |
| [`docs/specs/component/`](docs/specs/component/) | Component responsibilities and failure behavior |
| [`PHASED_IMPLEMENTATION_PLAN.md`](PHASED_IMPLEMENTATION_PLAN.md) | Phased implementation roadmap |
| [`docs/OPERATIONS.md`](docs/OPERATIONS.md) | Day-2 operations, monitoring, and troubleshooting |
| [`DESIGN.md`](DESIGN.md) | Design system tokens (colors, typography, spacing) |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Dev setup, code style, PR workflow |
| [`docs/release/PIPELINE.md`](docs/release/PIPELINE.md) | **Release engineering: read this first for any packaging/CI/CD/versioning work** |
| [`VERSION`](VERSION) | Single source of truth for release version |
| [`Makefile`](Makefile) | Packaging, testing, signing, and integration targets |
