# CHV

CHV is a Cloud Hypervisor management repository with a SvelteKit UI and an active Rust backend/control-plane direction.

## Repository Direction

- Active backend/control-plane language: Rust
- Active implementation paths: `/Cargo.toml`, `/cmd`, `/crates`, `/gen/rust`
- Authoritative contracts: `/proto`
- Authoritative architecture and component specs: `/docs/specs`
- Rust implementation guidance: `/docs/chv-llm-handoff-pack`
- Legacy Go prototype: `/legacy/go-controlplane` (reference only, non-authoritative)

If you are starting new backend or control-plane work, do not extend archived Go code. Start from the Rust workspace, proto contracts, and tracked spec packs instead.

## Active Paths

### Rust workspace

- `/cmd/chv-agent`
- `/cmd/chv-stord`
- `/cmd/chv-nwd`
- `/crates`
- `/gen/rust`

### Specs and contracts

- `/proto/controlplane/control-plane-node.proto`
- `/proto/node/chv-stord-api.proto`
- `/proto/node/chv-nwd-api.proto`
- `/docs/specs/adr`
- `/docs/specs/component`
- `/docs/specs/ops`
- `/docs/specs/proto`

## Development

Build the active backend workspace:

```bash
cargo build --workspace
```

Run the active backend test suite:

```bash
cargo test --workspace
```

Build the Web UI:

```bash
cd ui && npm run build
```

## Legacy Go Archive

The previous Go controller/agent prototype has been moved to `/legacy/go-controlplane` so it remains available for reference without appearing to be the current implementation path.

That archive keeps:

- the legacy Go module and source tree
- Go-specific Docker, compose, systemd, and install assets
- historical prototype docs that still have reference value

Do not treat anything in `/legacy/go-controlplane` as the current backend source of truth.

## Direction Reference

See `/REPOSITORY_DIRECTION.md` for the short repository-direction statement intended for both humans and LLM-guided implementation.
