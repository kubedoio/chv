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

## Skill Routing

When the user's request matches an available skill, ALWAYS invoke it using the Skill tool as your FIRST action. Do NOT answer directly, do NOT use other tools first.

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
- Save progress, checkpoint, resume → invoke context-save / context-restore
- Code quality, health check → invoke health

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (60-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk go test             # Go test failures only (90%)
rtk jest                # Jest failures only (99.5%)
rtk vitest              # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk pytest              # Python test failures only (90%)
rtk rake test           # Ruby test failures only (90%)
rtk rspec               # RSpec test failures only (60%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%). Format flags (-c, -l, -L, -o, -Z) run raw.
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /rtk-instructions -->