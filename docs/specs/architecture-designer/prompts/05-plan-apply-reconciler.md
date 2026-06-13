# Prompt 05: Implement Plan and Apply Reconciler

You are working in the CHV repository.

Goal:
Implement safe, idempotent plan/apply execution for Architecture Designer.

Tasks:

1. Add endpoints:
   - POST /architectures/{id}/plan
   - POST /architectures/{id}/apply
   - POST /architectures/{id}/destroy-plan
   - POST /architectures/{id}/destroy
2. Implement desired/current diff.
3. Generate plan actions:
   - create
   - update
   - delete
   - replace
   - noop
   - blocked
4. Add plan expiry after 15 minutes or inventory changes.
5. Require confirmation before apply.
6. Require typed architecture name for destructive plans.
7. Apply through CHV task system only.
8. Operation order:
   - roles
   - users
   - datastores
   - networks
   - images
   - templates
   - instances
   - disk attachments
   - network attachments
   - cloud-init users
   - backup policy bindings
9. Store apply run result and logs.
10. Mark architecture status after run.
11. Store drift baseline after successful apply.

Acceptance criteria:

- Plan shows create/update/delete summary.
- Apply creates linkable CHV task(s).
- Stale plan cannot be applied.
- Destructive plan requires typed confirmation.
- Failed apply records useful error result.
- Re-running apply is idempotent where possible.
