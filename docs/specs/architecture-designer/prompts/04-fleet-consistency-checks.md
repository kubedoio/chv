# Prompt 04: Implement Fleet Consistency Checks

You are working in the CHV repository.

Goal:
Implement live consistency checks between a desired CHVArchitecture topology and the current CHV fleet/resources.

Tasks:

1. Add endpoint:
   - POST /architectures/{id}/check-fleet
2. Use current inventory/resource APIs to check:
   - target host exists
   - host is healthy
   - host is schedulable
   - CPU capacity is sufficient
   - memory capacity is sufficient
   - datastore exists or can be created
   - datastore capacity is sufficient
   - network bridge exists or can be created
   - VLAN is available
   - IP address is free
   - image exists or source is available
   - backup target is reachable if defined
   - secret_ref exists
   - requesting user has deploy permission
3. Return finding contract with severity, code, message, path, resource_ref, blocking and suggestion.
4. Add UI panel showing results.
5. Prevent plan/apply if check result has blocking errors.
6. Keep warnings deployable only with explicit acknowledgement.

Acceptance criteria:

- Missing host produces HOST_NOT_FOUND.
- Insufficient memory produces INSUFFICIENT_MEMORY.
- Duplicate IP in current fleet produces IP_ALREADY_USED.
- Missing datastore produces DATASTORE_NOT_FOUND.
- Warnings are visible but not silently ignored.
