# CHV - Cloud Hypervisor Virtualization Platform

## Project Overview

CHV is a Go-based virtualization management platform with a Svelte frontend for managing VMs, networks, storage, and images.

## Architecture

- **Backend:** Go (chi router, SQLite, WebSocket)
- **Frontend:** SvelteKit with Svelte 5 runes
- **Agent:** Go binary running on compute nodes
- **Controller:** Go binary serving API + static UI

## Build Commands

```bash
# Backend
go build -o chv-controller ./cmd/chv-controller
go build -o chv-agent ./cmd/chv-agent

# Frontend
cd ui && npm run build
```

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
