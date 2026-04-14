# Legacy Go Cleanup Report

Date: 2026-04-14

## Phase 1: Audit

### Audit summary

- The repository already contains an active Rust workspace at `/Cargo.toml`, `/cmd/chv-agent`, `/cmd/chv-stord`, `/cmd/chv-nwd`, `/crates`, and `/gen/rust`.
- The repository still contains 99 Go source files plus Go-specific operational assets in active root paths.
- Root guidance still advertised Go as the active backend, which is unsafe for both human contributors and LLM-guided implementation.
- The authoritative architecture/spec direction is already Rust-first in:
  - `/docs/specs`
  - `/proto`
  - `/docs/chv-llm-handoff-pack`
  - `/docs/rust-controlplane-pack`

### Authoritative material to keep in active paths

These are intentionally not part of the archival/deletion set:

- `/Cargo.toml`, `/Cargo.lock`, `/cmd/chv-agent`, `/cmd/chv-stord`, `/cmd/chv-nwd`, `/crates`, `/gen/rust`
- `/proto/controlplane/control-plane-node.proto`
- `/proto/node/chv-stord-api.proto`
- `/proto/node/chv-nwd-api.proto`
- `/docs/specs/adr/*`
- `/docs/specs/component/*`
- `/docs/specs/ops/*`
- `/docs/specs/proto/*`
- `/docs/chv-llm-handoff-pack/*`
- `/docs/rust-controlplane-pack/*`

Note: `/docs/rust-controlplane-pack` is present in the current worktree but currently untracked, so it should not be treated as guaranteed committed repository state until it is intentionally added.
- `/ui/*`

### Legacy Go artifacts found and classification

| Paths | Category | Reason | Risk if kept untouched |
| --- | --- | --- | --- |
| `/go.mod`, `/go.sum` | archive | Root Go module makes the repository look Go-authored even though active implementation is Rust. | Humans and LLMs keep treating Go as the primary backend and may run `go build` from the repo root by default. |
| `/internal/**` (controller, agent, storage, auth, VM, bootstrap, networking, quota, health, metrics, DB, tests) | archive | These are the legacy Go backend/control-plane prototype implementation. They may still contain useful domain ideas, but they are no longer authoritative. | The most severe ambiguity source: LLMs will extend these packages instead of the Rust workspace, creating split-brain implementation. |
| `/pkg/**` | archive | Shared Go utility package set for the legacy prototype. | Keeps the Go module looking alive and reusable from active root paths. |
| `/cmd/chv-controller/main.go` | archive | Legacy Go controller entrypoint. | Strongly implies the active control plane is still Go. |
| `/cmd/chv-validator/main.go` | archive | Legacy CLI bound to the Go controller/agent APIs. Potentially useful as historical reference only. | Suggests the controller API and validation flow are still current and supported. |
| `/cmd/chv-agent/main.go` (ignored local file inside the Rust command path) | archive | Legacy Go agent entrypoint co-located with the active Rust `chv-agent`. | Particularly dangerous for LLMs and humans because the active Rust command path contains a shadow Go entrypoint. |
| `/Dockerfile.controller`, `/Dockerfile.agent` | archive | These Dockerfiles build Go binaries and represent the old prototype deployment path. | Operators or agents may containerize and deploy the legacy backend by mistake. |
| `/docker-compose.yml`, `/docker-compose.prod.yml` | archive | Both compose files stand up the Go controller path. | Strong operational ambiguity; they look like the default supported deployment path. |
| `/chv-controller.service`, `/chv-agent.service`, `/install.sh`, `/start-chv.sh`, `/scripts/build-remote.sh`, `/configs/chv.yaml`, `/configs/schema_sqlite.sql` | archive | Legacy Go prototype service management, install, remote build, and config/schema surface. | Makes the repo appear operationally centered around the Go controller/agent instead of the Rust workspace and specs. |
| `/docs/API_SPEC.md`, `/docs/E2E_TEST_RESULTS.md`, `/docs/VM_VALIDATION.md` | archive | Useful historical reference, but they document the Go prototype behavior and controller/agent flows. | Readers may treat them as active backend docs unless they are clearly marked and relocated. |
| `/CONNECTION_DETAILS.txt` | delete | Environment-specific runtime notes for the legacy Go deployment; not authoritative design knowledge. | Misleads readers with stale local hostnames, ports, processes, and startup steps. |
| `/controller-build.log` | delete | Generated build log artifact, not durable knowledge. | Noise in the root path and stale evidence of the legacy controller build. |
| `/ui-build-debug.log`, `/ui-build-error.log`, `/ui-build-full.log`, `/ui-build-verbose.log`, `/ui-dev.log` | delete | Tracked local build/dev logs that still mention legacy controller build flow. | Leaves stale root-level evidence that points readers back to the old Go-backed operational path. |
| `/chv-validator` | delete | Compiled Go binary checked into the repo root. | Suggests the legacy CLI is a shipped artifact and adds avoidable root-path clutter. |
| `/README.md`, `/CLAUDE.md` | rewrite documentation around | Both still instruct contributors to treat Go as the active backend. | New contributors and LLMs will keep landing work in the wrong language and wrong paths. |

### Notes on intentionally retained non-Go authority

- Proto contracts remain in active root paths because they are authoritative.
- ADRs and component specs remain in active root paths because they define the architecture and should continue guiding Rust implementation.
- The Rust control-plane pack remains in active docs because it explicitly defines the intended direction and migration workflow.

## Phase 2: Cleanup Plan

### Exact moves

Archive the legacy Go prototype under `/legacy/go-controlplane/`:

- Move `/go.mod` -> `/legacy/go-controlplane/module/go.mod`
- Move `/go.sum` -> `/legacy/go-controlplane/module/go.sum`
- Move `/internal/**` -> `/legacy/go-controlplane/module/internal/**`
- Move `/pkg/**` -> `/legacy/go-controlplane/module/pkg/**`
- Move `/cmd/chv-controller/main.go` -> `/legacy/go-controlplane/module/cmd/controller/main.go`
- Move `/cmd/chv-validator/main.go` -> `/legacy/go-controlplane/module/cmd/validator/main.go`
- Move `/cmd/chv-agent/main.go` -> `/legacy/go-controlplane/module/cmd/agent/main.go`
- Move `/Dockerfile.controller` -> `/legacy/go-controlplane/ops/Dockerfile.controller`
- Move `/Dockerfile.agent` -> `/legacy/go-controlplane/ops/Dockerfile.agent`
- Move `/docker-compose.yml` -> `/legacy/go-controlplane/ops/docker-compose.yml`
- Move `/docker-compose.prod.yml` -> `/legacy/go-controlplane/ops/docker-compose.prod.yml`
- Move `/chv-controller.service` -> `/legacy/go-controlplane/ops/chv-controller.service`
- Move `/chv-agent.service` -> `/legacy/go-controlplane/ops/chv-agent.service`
- Move `/install.sh` -> `/legacy/go-controlplane/ops/install.sh`
- Move `/start-chv.sh` -> `/legacy/go-controlplane/ops/start-chv.sh`
- Move `/scripts/build-remote.sh` -> `/legacy/go-controlplane/ops/scripts/build-remote.sh`
- Move `/configs/chv.yaml` -> `/legacy/go-controlplane/config/chv.yaml`
- Move `/configs/schema_sqlite.sql` -> `/legacy/go-controlplane/config/schema_sqlite.sql`
- Move `/docs/API_SPEC.md` -> `/legacy/go-controlplane/docs/API_SPEC.md`
- Move `/docs/E2E_TEST_RESULTS.md` -> `/legacy/go-controlplane/docs/E2E_TEST_RESULTS.md`
- Move `/docs/VM_VALIDATION.md` -> `/legacy/go-controlplane/docs/VM_VALIDATION.md`

Add archive context docs:

- Add `/legacy/go-controlplane/README.md`
- Add `/REPOSITORY_DIRECTION.md`

### Exact deletions

- Delete `/CONNECTION_DETAILS.txt`
- Delete `/controller-build.log`
- Delete `/ui-build-debug.log`
- Delete `/ui-build-error.log`
- Delete `/ui-build-full.log`
- Delete `/ui-build-verbose.log`
- Delete `/ui-dev.log`
- Delete `/chv-validator`

### Exact documentation updates

- Rewrite `/README.md` so the active backend/control-plane direction is Rust.
- Rewrite `/README.md` development guidance to point to the Rust workspace, active specs, and archived Go path.
- Rewrite `/CLAUDE.md` so future agents treat Rust/specs as source of truth and the archived Go tree as non-authoritative reference only.
- Add `/REPOSITORY_DIRECTION.md` with a short repository-direction statement for humans and LLMs.

### Exact CI/script/build updates

- Remove root-level Go build and Go deployment entrypoints by archiving:
  - `/Dockerfile.controller`
  - `/Dockerfile.agent`
  - `/docker-compose.yml`
  - `/docker-compose.prod.yml`
  - `/install.sh`
  - `/start-chv.sh`
  - `/scripts/build-remote.sh`
  - `/chv-controller.service`
  - `/chv-agent.service`
- No active GitHub Actions or root Makefile were found, so there is no CI workflow file to rewrite in this pass.

## Phase 3: Planned patch slices

1. Add this report and create the legacy archive scaffolding.
2. Move the Go module and entrypoints into the legacy archive.
3. Move Go-specific operational assets and historical docs into the legacy archive.
4. Delete the root generated artifacts and stale runtime notes.
5. Rewrite repository direction docs (`README.md`, `CLAUDE.md`, `REPOSITORY_DIRECTION.md`).

## Phase 4: Expected post-cleanup state

- The active root implementation path points to Rust and specs.
- Legacy Go code remains available for reference under a clearly non-authoritative archive path.
- Root operational assets no longer suggest the Go controller is the current backend.
- Proto contracts and architecture docs remain authoritative and easy to find.
