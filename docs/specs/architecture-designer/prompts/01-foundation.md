# Prompt 01: Implement Architecture Designer Foundation

You are working in the CHV repository.

Goal:
Implement the foundation for a new WebUI feature called Architecture Designer.

Context:
CHV currently has Fleet Overview in the left panel. Add a new DESIGN section above Fleet Overview with Architecture Designer and Saved Topologies. The feature must save topology drafts on the dashboard, but this prompt does not yet implement the visual Svelte Flow canvas.

Tasks:

1. Add backend data model for:
   - ArchitectureTopology
   - ArchitectureVersion
   - ArchitecturePlan
   - ArchitectureApplyRun
   - ArchitectureDriftReport
2. Add API routes:
   - GET /architectures
   - POST /architectures
   - GET /architectures/{id}
   - PUT /architectures/{id}
   - DELETE /architectures/{id}
3. Add frontend routes:
   - /architectures
   - /architectures/new
   - /architectures/{id}
4. Add left panel DESIGN group above Fleet Overview:
   - Architecture Designer
   - Saved Topologies
5. Add dashboard list/cards for saved topologies.
6. Add status enum:
   - draft
   - valid
   - invalid
   - planned
   - applying
   - applied
   - drifted
   - failed
   - archived
7. Store design_graph_json and latest_yaml as nullable fields for now.
8. No fake production values.
9. Keep code typed, modular and testable.

Acceptance criteria:

- `/architectures` lists saved topologies.
- User can create a draft topology with name, description and environment.
- Draft appears in the left/dashboard navigation.
- Existing Fleet Overview and infrastructure tree continue to work.
- No deploy/apply logic is implemented in this step.
