# CHV

CHV is a Linux-first, cloud-image-first virtualization platform for sovereign private cloud and edge cloud environments. It provides API-driven VM lifecycle management built on [Cloud Hypervisor](https://www.cloudhypervisor.org/).

## Architecture

- **Backend / Control Plane**: Rust (active)
- **Frontend**: SvelteKit Web UI
- **Contracts**: gRPC/protobuf

## Repository Structure

```
.
├── cmd/                    # Rust binaries
│   ├── chv-agent/          # Node agent (VM lifecycle, CHV runtime)
│   ├── chv-controlplane/   # Control plane (orchestration, node mgmt)
│   ├── chv-nwd/            # Network daemon
│   └── chv-stord/          # Storage daemon
├── crates/                 # Rust library crates
│   ├── chv-common/
│   ├── chv-config/
│   ├── chv-controlplane-*
│   ├── chv-agent-*
│   ├── chv-nwd-core/
│   ├── chv-stord-*
│   ├── chv-errors/
│   └── chv-observability/
├── gen/rust/               # Generated Rust code from proto contracts
├── proto/                  # Authoritative gRPC/protobuf contracts
│   ├── controlplane/
│   ├── node/
│   └── webui/
├── ui/                     # SvelteKit Web UI
│   └── src/
│       ├── routes/         # Page routes
│       └── lib/            # Components, stores, API clients
└── docs/                   # Specs, ADRs, and design docs
    ├── specs/
    └── webui-spec-pack/
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/)
- [Node.js + npm](https://nodejs.org/) (for the Web UI)

### Build the Rust workspace

```bash
cargo build --workspace
```

### Run Rust tests

```bash
cargo test --workspace
```

### Build the Web UI

```bash
cd ui && npm install && npm run build
```

### Run the Web UI in development mode

```bash
cd ui && npm run dev
```

## Key Documentation

- **Deployment Guide**: [`docs/DEPLOYMENT.md`](./docs/DEPLOYMENT.md) — how to deploy CHV on a combined control-plane + hypervisor host
- **Repository Direction**: [`REPOSITORY_DIRECTION.md`](./REPOSITORY_DIRECTION.md)
- **Design System**: [`DESIGN.md`](./DESIGN.md)
- **Architecture Decision Records**: [`docs/specs/adr/`](./docs/specs/adr)
- **Component Specs**: [`docs/specs/component/`](./docs/specs/component)
- **WebUI Spec Pack**: [`docs/webui-spec-pack/`](./docs/webui-spec-pack)
- **Changelog**: [`CHANGELOG.md`](./CHANGELOG.md)

## Version

Current version: `0.0.0.2` (see [`VERSION`](./VERSION))

## Direction

> Rust is the active backend and control-plane language for CHV. New backend work belongs in the Rust workspace (`/cmd`, `/crates`, `/gen/rust`), driven by the proto contracts in `/proto` and the specs in `/docs/specs`.
