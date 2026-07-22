# CellHV Core Post-194 Correctness Audit

## Context

This document audits the correctness and truthful capability of the CellHV Core components following the merge of PRs #190 through #194. The focus is on aligning implementation claims with verified functionality.

## PR #190: Foundation Layout
- **Audit:** Directory structure and base types laid out. Types are well-encapsulated. No capability claims are overstated here.

## PR #191: Store & Journal
- **Audit:** Introduced SQLite persistence and the durable operation journal.
- **Findings:** Found duplicate resource-version assignment bugs and delete-all-and-reinsert logic in attachment topology persistence. These bugs have been repaired to strictly enforce resource-version increments.

## PR #192: Operations & Executor
- **Audit:** Implemented the stateless operation validation engine and `JournalExecutor`.
- **Findings:** Claimed support for dynamic attach/detach operations, which are unsupported for Core M1. These have been repaired to fail safely before state modification.

## PR #193: API Listener & Capabilities
- **Audit:** Added native Unix domain socket listener and fake adapters.
- **Findings:** Documentation overstated the "unwired" status. We have clarified that the `CoreRuntimeOwner` is now composed with a real executor in production, though KVM evidence remains at T3.

## PR #194: Runtime Ownership
- **Audit:** Addressed process lifecycle and component shutdown.
- **Findings:** The use of placeholder error types (like `ListenerError::DrainTimeout(0)`) was misleading and hindered debugging. We have updated `RuntimeOwnerError` to provide explicit stage startup and shutdown errors.

## Conclusion

All correctness bugs identified in the audit of PRs #190–#194 have been addressed in the S1 branch. We have successfully repaired attachment semantics, error modeling, and documentation to reflect true capabilities.
