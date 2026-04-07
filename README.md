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

The repository is mid-normalization toward the consolidated MVP-1 spec. The active slices currently focus on:

- SQLite schema and repository foundation
- token authentication with hashed opaque bearer tokens
- install status inspection
- bootstrap and repair actions for `/var/lib/chv/`, `chvbr0`, and `localdisk`
- SvelteKit operator console prepared for container deployment

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

## Web UI Container

The web UI can run as a standalone container when you want to point it at an already-running controller. The agent still stays host-native on the hypervisor node.

Build:

```bash
docker build -t chv-ui:latest ./ui
```

Run:

```bash
docker run --rm -p 3000:3000 \
  -e PUBLIC_CHV_API_BASE_URL=http://10.5.199.83:8080/api/v1 \
  chv-ui:latest
```

`PUBLIC_CHV_API_BASE_URL` should point at the host-native controller API that the browser can actually reach.

## Compose Stack

For a simpler control-plane deployment, the repository includes [docker-compose.yml](/Users/scolak/Projects/chv/docker-compose.yml) for:

- `chv-controller` in a container
- `chv-webui` in a container

`chv-agent` is intentionally not part of this compose stack. It must run host-native on the hypervisor node that provides Cloud Hypervisor and bridge/TAP access.

Start the stack:

```bash
PUBLIC_CHV_API_BASE_URL=http://10.5.199.83:8080/api/v1 \
CHV_WEBUI_PORT=3100 \
docker compose up -d --build
```

Useful overrides:

- `CHV_CONTROLLER_PORT` defaults to `8080`
- `CHV_WEBUI_PORT` defaults to `3000`
- `PUBLIC_CHV_API_BASE_URL` defaults to `http://localhost:8080/api/v1`

On a remote server, set `PUBLIC_CHV_API_BASE_URL` to the server IP or DNS name the browser will use, not the internal Compose service name. The default `localhost` value is only suitable when the browser is running on the same machine as the compose stack.

## Design

The intended UI language is defined in [DESIGN.md](/Users/scolak/Projects/chv/DESIGN.md). The console remains light-first, restrained, border-heavy, and operator-oriented.
