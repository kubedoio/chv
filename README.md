# CHV MVP-1

CHV is a Linux-first, cloud-image-first virtualization platform built on Cloud Hypervisor.

This repository is being normalized around one active product direction only:

- Frontend: SvelteKit + TypeScript
- Backend: Go
- Database: SQLite
- Hypervisor: Cloud Hypervisor
- Host networking: Linux bridge
- Default bridge: `chvbr0`
- Default bridge IP: `10.0.0.1/24`
- Default data root: `/var/lib/chv/`
- Default storage backend: `localdisk`
- Default image format: `qcow2`
- Cloud-init delivery: seed ISO generated before boot

## Scope

Included in MVP-1:

- Linux VMs only
- explicit install/bootstrap subsystem
- SQLite-backed control-plane state
- localdisk storage
- qcow2 image import
- cloud-init seed ISO preparation
- VM create/start/stop/delete
- operator web UI
- opaque bearer API tokens

Excluded from MVP-1:

- PostgreSQL
- Vue / PrimeVue
- node scheduler / reconciliation / quota subsystems
- NFS, Ceph, DRBD, distributed storage
- raw-runtime-disk-first provisioning
- JWT auth assumptions
- advanced SDN or overlay networking

## Current Rebuild Status

The repository is mid-normalization toward the consolidated MVP-1 spec. The active backend slice currently focuses on:

- SQLite schema and repository foundation
- token authentication with hashed opaque bearer tokens
- install status inspection
- bootstrap and repair actions for `/var/lib/chv/`, `chvbr0`, and `localdisk`

## Development

Controller:

```bash
/usr/local/go/bin/go run ./cmd/chv-controller
```

Agent:

```bash
/usr/local/go/bin/go run ./cmd/chv-agent
```

Install status API:

```bash
curl http://localhost:8080/api/v1/install/status
```

Create a token:

```bash
curl -X POST http://localhost:8080/api/v1/tokens \
  -H "Content-Type: application/json" \
  -d '{"name":"admin"}'
```

## Design

The intended UI language is defined in [DESIGN.md](/Users/scolak/Projects/chv/DESIGN.md). The console remains light-first, restrained, border-heavy, and operator-oriented.

