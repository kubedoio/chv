# Specification: Implementation Roadmap

## Phase 0: Contracts and skeleton

Deliverables:

- Add Architecture Designer routes.
- Add data model migrations.
- Add API skeleton.
- Add empty dashboard list.
- Add placeholder designer page.

Acceptance:

- `/architectures` loads.
- A topology draft can be created and listed.

## Phase 1: YAML model and validation

Deliverables:

- Implement `CHVArchitecture` parser.
- Implement JSON Schema validation.
- Implement reference validation.
- Add YAML editor.
- Add YAML export.

Acceptance:

- Valid example passes.
- Invalid references produce stable error codes.
- Raw secrets are rejected.

## Phase 2: Svelte Flow designer

Deliverables:

- Add Svelte Flow dependency.
- Implement node palette.
- Implement custom nodes.
- Implement edge rules.
- Implement inspector panel.
- Implement graph-to-model conversion.
- Implement model-to-graph conversion.

Acceptance:

- User can visually create host, network, datastore and instance.
- YAML is generated from canvas.
- YAML import recreates canvas.

## Phase 3: Fleet consistency checks

Deliverables:

- Compare desired state with current CHV inventory.
- Check host health/capacity.
- Check network/IP conflicts.
- Check datastore capacity.
- Check images/secrets/backup target existence.

Acceptance:

- UI shows errors/warnings grouped by resource.
- Blocking findings prevent plan/apply.

## Phase 4: Plan generation

Deliverables:

- Implement desired/current diff.
- Create plan result contract.
- Show plan preview UI.
- Add plan expiry.

Acceptance:

- Plan lists create/update/delete actions.
- Destructive changes require confirmation.

## Phase 5: Apply/reconcile

Deliverables:

- Implement task-gated apply.
- Implement operation ordering.
- Store apply run status/logs.
- Update architecture status.

Acceptance:

- Applying a simple topology creates resources.
- Every mutation creates CHV tasks.
- Failed apply shows clear result.

## Phase 6: Drift detection

Deliverables:

- Store baseline after successful apply.
- Compare baseline against current resources.
- Show drift status on topology dashboard.

Acceptance:

- Manually changed resource is reported as drift.
- Drift report links to affected resources.

## Phase 7: Hardening

Deliverables:

- E2E tests.
- Permission tests.
- Destructive confirmation tests.
- Stale plan tests.
- Import/export tests.
- Large graph performance test.

Acceptance:

- Feature is safe for production preview.
