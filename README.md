# CHV

CHV is a Linux-first, cloud-image-first virtualization platform for sovereign private cloud and edge cloud environments. It provides API-driven VM lifecycle management built on [Cloud Hypervisor](https://www.cloudhypervisor.org/).

## Current Phase

**Version:** `0.0.0.2`  
**Phase:** Early-to-MVP transitioning to stability  

The project has a solid Phase 1 foundation (Rust control plane, SQLite store, certificate enrollment, gRPC services) and a functional SvelteKit Web UI. Active work is tracked in the [Phased Implementation Plan](./PHASED_IMPLEMENTATION_PLAN.md) covering stability hardening, feature completion, and production readiness.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Web UI                              │
│                     (SvelteKit + Tailwind)                  │
└─────────────────────────────┬───────────────────────────────┘
                              │ HTTP / BFF
┌─────────────────────────────▼───────────────────────────────┐
│                   chv-controlplane                            │
│         (Orchestration · SQLite · BFF HTTP · gRPC)          │
└─────────────────────────────┬───────────────────────────────┘
                              │ mTLS gRPC
┌─────────────────────────────▼───────────────────────────────┐
│                      chv-agent                                │
│           (VM lifecycle · CHV runtime · Serial console)     │
├─────────────────────────────┬───────────────────────────────┤
│         chv-stord           │           chv-nwd             │
│   (Volumes · Pools · Images)│  (Networks · Firewall · NAT)  │
└─────────────────────────────┴───────────────────────────────┘
```

- **Backend / Control Plane**: Rust (Tokio, tonic, axum, sqlx/SQLite)
- **Frontend**: SvelteKit 2 + Svelte 5 + TailwindCSS + Vite
- **Contracts**: gRPC/protobuf
- **Metrics**: Prometheus endpoint

## Repository Structure

```
.
├── cmd/                    # Rust binaries
│   ├── chv-agent/          # Node agent (VM lifecycle, CHV runtime, serial console)
│   ├── chv-controlplane/   # Control plane (orchestration, node mgmt, enrollment, BFF)
│   ├── chv-nwd/            # Network daemon (bridge, netns, nftables, DHCP, DNS)
│   └── chv-stord/          # Storage daemon (volumes, pools, images, snapshots)
├── crates/                 # Rust library crates
│   ├── chv-common/
│   ├── chv-config/
│   ├── chv-controlplane-*
│   ├── chv-agent-*
│   ├── chv-nwd-core/
│   ├── chv-stord-*
│   ├── chv-webui-bff/
│   ├── chv-errors/
│   └── chv-observability/
├── gen/rust/               # Generated Rust code from proto contracts
├── proto/                  # Authoritative gRPC/protobuf contracts
│   ├── controlplane/       # Control plane ↔ Agent
│   ├── node/               # Agent ↔ Storage/Network
│   └── webui/              # Web UI ↔ BFF
├── ui/                     # SvelteKit Web UI
│   └── src/
│       ├── routes/         # Page routes
│       └── lib/            # Components, stores, API clients
├── scripts/                # Install and build scripts
├── deploy/                 # Deployment artifacts (entrypoint, systemd)
└── docs/                   # Specs, ADRs, design docs, and plans
    ├── specs/adr/          # Architecture Decision Records
    ├── specs/component/    # Component specifications
    ├── plans/              # Sprint and implementation plans
    └── examples/           # Example configs (TOML, nginx, systemd)
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+ and npm
- `protobuf-compiler` (for regenerating `gen/rust/` from `proto/`)

### Quick Start

```bash
# Build the entire workspace
cargo build --workspace

# Run all Rust tests
cargo test --workspace

# Build the Web UI
cd ui && npm install && npm run build

# Run the Web UI dev server
cd ui && npm run dev
```

### Makefile Shortcuts

| Command | Description |
|---------|-------------|
| `make build` | Release build of the Rust workspace |
| `make build-ui` | Install deps and build the UI |
| `make test` | Run Rust tests |
| `make fmt` | Format all Rust code |
| `make release` | Create a release tarball (`dist/`) |
| `make dev-install` | Install systemd services for local development |
| `make clean` | Clean build artifacts |

### Proto Changes

If you modify `.proto` files, regenerate the Rust bindings:

```bash
# The workspace uses tonic-build; typically a rebuild is sufficient:
cargo build --workspace
```

## Key Documentation

| Document | Purpose |
|----------|---------|
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | System architecture, data flow, and boundaries |
| [`docs/DEPLOYMENT.md`](./docs/DEPLOYMENT.md) | Deploy CHV on a combined control-plane + hypervisor host |
| [`docs/specs/adr/`](./docs/specs/adr) | Architecture Decision Records (001–010) |
| [`docs/specs/component/`](./docs/specs/component) | Component specs (agent, stord, nwd) |
| [`PHASED_IMPLEMENTATION_PLAN.md`](./PHASED_IMPLEMENTATION_PLAN.md) | Phased implementation roadmap |
| [`docs/OPERATIONS.md`](./docs/OPERATIONS.md) | Day-2 operations, monitoring, and troubleshooting |
| [`DESIGN.md`](./DESIGN.md) | Design system (typography, color, spacing, dark mode) |
| [`CHANGELOG.md`](./CHANGELOG.md) | Release history |
| [`CONTRIBUTING.md`](./CONTRIBUTING.md) | Development workflow, code style, and PR process |
| [`CLAUDE.md`](./CLAUDE.md) | Agent orientation and build rules |

## CI / CD

GitHub Actions runs on every push and PR:
- `cargo check`, `cargo clippy -- -D warnings`, `cargo test --workspace`
- UI `npm ci`, `npm run build`, and `npm run check` (if present)

See [`.github/workflows/ci.yml`](./.github/workflows/ci.yml).

## Version

Current version: `0.0.0.2` (see [`VERSION`](./VERSION))

## Direction

> Rust is the active backend and control-plane language for CHV. New backend work belongs in the Rust workspace (`/cmd`, `/crates`, `/gen/rust`), driven by the proto contracts in `/proto` and the specs in `/docs/specs`.
