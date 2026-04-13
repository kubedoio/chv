# CHV LLM Handoff Pack

This pack is the next layer after the `/specs` tree.

It contains:
- `repository-layout-spec.md` — recommended repo structure and ownership boundaries
- `prompts/chv-stord-implementation-prompt.md` — first implementation prompt for `chv-stord`
- `prompts/chv-nwd-implementation-prompt.md` — first implementation prompt for `chv-nwd`
- `prompts/chv-agent-implementation-prompt.md` — first implementation prompt for `chv-agent`
- `prompts/reviewer-audit-prompt.md` — strict review prompt for implementation output
- `guides/llm-workflow-guide.md` — practical step-by-step guide for running the work with one or more LLMs

Recommended execution order:
1. `chv-stord`
2. `chv-nwd`
3. `chv-agent`
4. reviewer/audit pass
5. integration and repo cleanup
