# Prompt 03: Implement CHVArchitecture YAML Schema and Validator

You are working in the CHV repository.

Goal:
Implement `CHVArchitecture` YAML generation, import, export and validation.

Contract:

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
```

Tasks:

1. Implement a parser for CHVArchitecture YAML.
2. Implement JSON Schema validation based on `schemas/chvarchitecture-v1alpha1.schema.json`.
3. Implement graph -> normalized model -> YAML generation.
4. Implement YAML -> normalized model -> graph reconstruction.
5. Add YAML mode and split mode to the UI.
6. Add export YAML button.
7. Add import YAML action.
8. Implement static validation checks:
   - schema valid
   - duplicate names
   - missing references
   - invalid CIDRs
   - overlapping networks
   - duplicate IPs
   - IP outside selected network
   - invalid datastore/image/template references
   - invalid user/role references
   - raw secrets forbidden
9. Return stable finding codes.
10. Show validation results in UI grouped by resource.

Acceptance criteria:

- Example YAML imports successfully.
- Canvas edits regenerate YAML.
- YAML edits can regenerate the canvas.
- Invalid YAML produces clear errors.
- Raw secrets are rejected.
- Validation endpoint works without live fleet access.
