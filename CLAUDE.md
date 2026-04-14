# CHV - Cloud Hypervisor Virtualization Platform

## Project Overview

CHV is a Rust-first virtualization management repository with a Svelte frontend and proto/spec-driven backend direction.

## Repository Direction

- Active backend/control-plane language: Rust
- Active backend workspace: `/Cargo.toml`, `/cmd`, `/crates`, `/gen/rust`
- Authoritative contracts: `/proto`
- Authoritative design and behavior docs: `/docs/specs`, `/docs/chv-llm-handoff-pack`

## Build Commands

```bash
# Active backend workspace
cargo build --workspace
cargo test --workspace

# Frontend
cd ui && npm run build
```

## Backend Implementation Rules

- New backend/control-plane work belongs in the Rust workspace, not the archived Go tree.
- Proto contracts in `/proto` are the source of truth for inter-service APIs.
- ADRs and component specs in `/docs/specs` define the intended system boundaries.

## Skill routing

When the user's request matches an available skill, ALWAYS invoke it using the Skill
tool as your FIRST action. Do NOT answer directly, do NOT use other tools first.
The skill has specialized workflows that produce better results than ad-hoc answers.

Key routing rules:
- Product ideas, "is this worth building", brainstorming → invoke office-hours
- Bugs, errors, "why is this broken", 500 errors → invoke investigate
- Ship, deploy, push, create PR → invoke ship
- QA, test the site, find bugs → invoke qa
- Code review, check my diff → invoke review
- Update docs after shipping → invoke document-release
- Weekly retro → invoke retro
- Design system, brand → invoke design-consultation
- Visual audit, design polish → invoke design-review
- Architecture review → invoke plan-eng-review
- Save progress, checkpoint, resume → invoke checkpoint
- Code quality, health check → invoke health
