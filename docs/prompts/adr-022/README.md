# ADR-022 Dual VM Boot Model Implementation Prompts

This prompt pack turns ADR-022 and the CHV VM boot contract v1 into bounded
implementation handoffs.

It starts from the repository's current state. It does not redesign the boot
model — ADR-022 and its contract are the design authority.

## Source of truth

Before executing any prompt, read:

- `docs/specs/adr/022-dual-vm-boot-model.md`
- `docs/specs/contracts/chv-vm-boot-contract-v1.md`
- `docs/specs/ops/cloud-hypervisor-reference.md`
- `docs/governance/DOCUMENTATION_STANDARD.md`

## Standing rules

- Use the current qualified Cloud Hypervisor version. Do not upgrade the VMM
  as part of this work. (The D6 VMM re-qualification campaign owns pin moves.)
- `chv-agent` remains the VM lifecycle authority; Cloud Hypervisor remains the
  only active VMM backend.
- Do not edit frozen evidence, released history, old plans, analysis
  documents, or previously executed prompts.
- Do not silently fall back between boot modes.
- Firmware boot keeps its existing evidence state. Direct kernel boot starts
  at `DESIGN-ONLY` and must not be claimed above `CODE-SUPPORTED, UNQUALIFIED`
  until its own real-KVM evidence exists, per the contract's capability tiers.

## Prompts

1. [`01-implement-dual-vm-boot-model.md`](01-implement-dual-vm-boot-model.md) —
   implement the two explicit boot modes (`firmware`, `direct_kernel`) with
   content-addressed boot artifacts, keeping firmware boot working.
