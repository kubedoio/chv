# Legacy Go Control-Plane Archive

This directory preserves the previous Go controller/agent prototype for reference only.

## Status

- Non-authoritative
- Not the active backend path
- Kept only for historical design knowledge, migration context, and prototype reference

## Source of Truth

For active backend and control-plane work, use:

- `/Cargo.toml`
- `/cmd`
- `/crates`
- `/gen/rust`
- `/proto`
- `/docs/specs`
- `/docs/chv-llm-handoff-pack`

## Archive Layout

- `/legacy/go-controlplane/module` contains the archived Go module and source tree.
- `/legacy/go-controlplane/ops` contains archived Docker, compose, systemd, install, and remote-build assets.
- `/legacy/go-controlplane/config` contains archived prototype config/schema assets.
- `/legacy/go-controlplane/docs` contains archived historical prototype docs.

Do not move this archive back into active root paths unless the work is an explicit migration task.
