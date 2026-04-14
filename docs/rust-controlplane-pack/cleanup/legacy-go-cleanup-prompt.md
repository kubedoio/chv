# Legacy Go Cleanup Prompt

You are a senior repository migration and cleanup engineer.

The repository contains legacy Go control-plane experiments, but the authoritative backend direction is now Rust.

Your job is to design and execute a cleanup plan that makes the repository safe for continued Rust development and safe for LLM-guided implementation.

## Objectives

1. identify all Go artifacts related to abandoned or exploratory control-plane work
2. classify them into:
   - keep as reference
   - archive
   - delete
   - rewrite documentation around
3. ensure the active implementation path clearly points to Rust
4. remove ambiguity for humans and LLMs
5. preserve any still-useful design knowledge

## Important rules

- do not delete architecture docs or proto contracts that are still authoritative
- do not delete useful examples without first classifying them
- do not leave legacy Go code in active root paths if it is no longer authoritative
- if code is archived, move it under a clearly named legacy/archive directory
- update README and developer docs to reflect the Rust direction
- flag CI, scripts, Makefiles, Dockerfiles, and tests that still imply Go is the active control-plane backend
- produce a cleanup report before applying destructive changes

## Required output

### Phase 1: audit
Produce:
- Go files/modules found
- category for each
- reason
- risk if kept untouched

### Phase 2: cleanup plan
Produce:
- exact moves
- exact deletions
- exact doc updates
- exact CI/script updates

### Phase 3: patch set
Apply the cleanup in small, reviewable patches.

### Phase 4: report
Summarize:
- what was archived
- what was deleted
- what remains intentionally
- what follow-up manual work is still needed

## Extra requirement

At the end, produce a short `REPOSITORY_DIRECTION.md` draft that states:
- Rust is the active backend/control-plane language
- Go code in legacy paths is non-authoritative
- the source of truth lives in the Rust workspace and specs
