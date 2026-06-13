# Prompt 07: Validate Complete Architecture Designer Feature

You are working in the CHV repository.

Goal:
Perform a full feature validation of Architecture Designer and produce an honest report.

Validation scope:

1. Navigation and UI placement
2. Saved topology CRUD
3. Svelte Flow canvas
4. Graph save/load
5. YAML import/export
6. Graph <-> YAML synchronization
7. Static validation
8. Fleet consistency checks
9. Plan generation
10. Apply/task integration
11. Destructive confirmation
12. Permission enforcement
13. Drift detection
14. Existing WebUI regression

Required test cases:

- create empty topology
- create host/network/datastore/instance topology
- invalid edge rejected
- duplicate network CIDR rejected
- duplicate static IP rejected
- missing datastore rejected
- raw secret rejected
- YAML import creates graph
- graph edit updates YAML
- plan requires confirmation
- stale plan cannot apply
- destructive plan requires typed name
- apply creates task
- drift detected after manual resource change
- user without apply permission cannot deploy

Output:

Create a validation report with:

```text
GO / NO-GO
critical blockers
major issues
minor issues
regressions
security concerns
missing tests
recommended next steps
```

Be strict. Do not mark GO if deploy can happen without validation/plan/confirmation.
