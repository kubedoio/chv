# WebUI Review Prompt

> **⚠️ This is a curated review snapshot, not the canonical source of truth.**
>
> The authoritative versions of these ADRs, specs, and proto files live in [`docs/specs/`](../specs/).
> This pack exists only for LLM-driven review workflows. Do not edit these files directly;
> make changes in `docs/specs/` and regenerate the pack instead.
>
> Files unique to this pack (not in `docs/specs/`):
> - `README.md` (this file)
> - `guides/webui-implementation-guide.md`

Review the supplied WebUI implementation against the CHV WebUI ADRs, specs, and contracts.

Be strict.

## Reference material
- **ADRs**: `adr/001-webui-product-principles.md`, `adr/002-webui-architecture-boundary.md`, `adr/003-webui-navigation-model.md`, `adr/004-webui-task-and-state-model.md`, `adr/005-webui-design-system-direction.md`
- **Specs**: `spec/webui-product-spec.md`, `spec/webui-information-architecture.md`, `spec/webui-state-and-tasks-spec.md`, `spec/webui-api-bff-spec.md`, `spec/webui-design-system-spec.md`, `spec/webui-implementation-spec.md`
- **Implementation root**: `../../ui/src/`
- **Routes**: `../../ui/src/routes/`
- **Lib**: `../../ui/src/lib/`

## Find and report
1. architecture boundary violations
2. places where browser code talks to the wrong backend layer
3. state-model mismatches
4. task-model mismatches
5. missing degraded/failed/empty/loading states
6. design-system inconsistencies
7. page-specific duplication that should be reusable
8. missing tests
9. weak mutation UX
10. visual direction mismatches with the spec

## Review rules
- do not rewrite the code yet
- cite exact files and components
- separate blockers from improvements
- prefer concrete fixes

## Output format

### Blockers
- issue
- why it violates the spec
- exact fix direction

### Important fixes
- issue
- risk
- fix direction

### Nice-to-have improvements
- issue
- benefit
