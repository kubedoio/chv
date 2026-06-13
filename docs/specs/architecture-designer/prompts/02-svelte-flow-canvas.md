# Prompt 02: Implement Svelte Flow Architecture Designer Canvas

You are working in the CHV repository.

Goal:
Implement the Architecture Designer visual canvas using Svelte Flow.

Important:
Do not modify the existing live TopologyCanvas into an editor. The existing topology canvas remains for current fleet visualization. Build a separate Architecture Designer canvas.

Tasks:

1. Add Svelte Flow dependency if not already present.
2. Create component namespace:
   - ArchitectureDesignerPage.svelte
   - ArchitectureDesignerCanvas.svelte
   - ArchitectureNodeHost.svelte
   - ArchitectureNodeNetwork.svelte
   - ArchitectureNodeDatastore.svelte
   - ArchitectureNodeImage.svelte
   - ArchitectureNodeTemplate.svelte
   - ArchitectureNodeInstance.svelte
   - ArchitectureNodeUser.svelte
   - ArchitectureNodeRole.svelte
   - ArchitectureInspectorPanel.svelte
   - ArchitectureYamlPanel.svelte
   - ArchitectureValidationPanel.svelte
3. Add node palette with MVP node types:
   - Host
   - Network
   - Datastore
   - Image
   - Template
   - Instance
   - User
   - Role
4. Implement drag/drop from palette to canvas.
5. Implement edge creation and edge validation.
6. Allow only valid edge types:
   - Instance -> Host: placed_on
   - Instance -> Network: attached_to_network
   - Instance -> Datastore: uses_datastore
   - Template -> Image: uses_image
   - Instance -> Template: uses_template
   - User -> Role: has_role
7. Implement right-side inspector for selected node/edge.
8. Store graph JSON into ArchitectureTopology.design_graph_json.
9. Add Save Draft button.
10. Add basic visual validation badges on nodes.

Acceptance criteria:

- User can create a topology visually.
- Invalid connections are rejected immediately.
- Selected nodes are editable in inspector.
- Graph can be saved and reloaded.
- Existing live fleet topology remains untouched.
