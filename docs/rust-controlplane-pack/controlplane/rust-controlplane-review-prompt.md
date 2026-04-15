# Rust Control Plane Review Prompt

Review the supplied Rust control-plane implementation against the CHV specs.

Be strict.

## Find and report

1. architecture violations
2. contract mismatches against proto definitions
3. state-machine violations
4. persistence-model problems
5. places where desired and observed state were mixed incorrectly
6. unsafe or weak error handling
7. missing idempotency
8. missing tests
9. missing audit/event persistence
10. places where Cloud Hypervisor access leaked into the control plane

## Review rules

- do not rewrite the code yet
- do not soften findings
- cite exact files/functions
- separate blockers from improvements
- prefer concrete fixes over general advice

## Output format

### Blockers
- issue
- why it violates the spec
- exact fix direction

### Important fixes
- issue
- risk
- exact fix direction

### Nice-to-have improvements
- issue
- benefit
