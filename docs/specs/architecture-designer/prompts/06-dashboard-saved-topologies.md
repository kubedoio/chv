# Prompt 06: Implement Saved Topologies Dashboard and Drift View

You are working in the CHV repository.

Goal:
Expose saved Architecture Designer topologies as first-class dashboard objects with validation, plan, apply and drift status.

Tasks:

1. Improve `/architectures` dashboard cards.
2. Show:
   - name
   - environment
   - status
   - validation status
   - fleet check status
   - drift status
   - instances count
   - networks count
   - datastores count
   - last applied version
   - last apply task
   - updated time
3. Add filtering by:
   - environment
   - status
   - drift status
   - owner
4. Add `/architectures/{id}/runs` history page.
5. Add `/architectures/{id}/drift` page.
6. Implement read-only drift detection:
   - missing_resource
   - unexpected_resource
   - field_changed
   - capacity_changed
   - network_changed
   - permission_changed
   - attachment_changed
7. Link drift findings to affected live resources.
8. Add status badge in left panel for drifted topologies.

Acceptance criteria:

- Saved topologies are visible and searchable.
- Drifted topologies are obvious.
- Last apply task is clickable.
- Run history is preserved.
- Drift check does not mutate infrastructure.
