# CHV Prompt Packs

This directory holds the repository's bounded execution prompt packs. Each
pack turns an accepted design direction (or an active campaign) into a
sequence of handoffs with explicit invariants, evidence requirements, and
review gates.

Read a pack's `README.md` before executing any prompt in it.

## Packs

1. [`cellhv-core/`](cellhv-core/README.md) — phased implementation of the
   CellHV Core decisions (ADR-015/016/017): evolve `chv-agent` in place into
   the single-operation-engine runtime.
2. [`production-readiness/`](production-readiness/README.md) — the campaign
   that took CHV from technical preview to the v0.3.0-rc1 production
   candidate: safety, authority, reproducibility, qualification, release,
   and field-evidence legs.
3. [`adr-022/`](adr-022/README.md) — implementation of ADR-022, the dual VM
   boot model (`firmware` / `direct_kernel`) and the CHV VM boot contract v1.

## Standing rules across packs

- Prompts are executed as narrow, independently reviewed PRs.
- Do not edit frozen evidence, released history, old plans, analysis
  documents, or previously executed prompts.
- Capability claims follow the repository's tier vocabulary; a merged PR is
  not automatically qualification evidence.
