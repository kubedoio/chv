# Goal — First Credible CHV Production Candidate

Take the current `kubedoio/chv` `main` from technical preview to the **first credible production candidate** by reducing uncertainty rather than expanding feature scope.

## Required outcome

Produce one exact, reviewable CHV release candidate for which all of the following are true:

1. **Network safety:** CHV firewall/default-deny policy is scoped only to CHV-owned guest traffic and cannot break unrelated host, container, CNI, SSH, routing, or forwarded traffic.
2. **Single lifecycle authority:** Create/Start/Stop/Reboot/Delete enter one durable CellHV Core acceptance path before provider side effects; compatibility state is a projection, not a second authority.
3. **Fail-closed startup:** invalid security configuration fails through typed startup validation before listeners/services are constructed; no operator-controlled production path relies on panic behavior or weakens mTLS.
4. **Reproducible engineering baseline:** the Rust toolchain, generated-code policy, dependency advisory policy, security contact, review/protection expectations, and release inputs are explicit and reproducible.
5. **Real infrastructure evidence:** the exact candidate passes real KVM lifecycle/recovery and the supported multi-host migration/network/storage paths, including negative and interruption cases.
6. **Release evidence:** the exact qualified commit is packaged, signed/attested, published as an RC, downloaded through the public release path, installed on clean supported hosts, upgraded, and where supported rolled back.
7. **Reference deployment evidence:** the published artifacts run a bounded real deployment long enough to exercise restart, host reboot, management-plane outage, migration, backup/restore, upgrade, observability, and cleanup behavior.

## Operating principle

Do not optimize for number of features closed.

Optimize for this chain:

```text
CODED
  -> CI-VERIFIED
  -> KVM-VERIFIED
  -> MULTI-HOST-VERIFIED
  -> RELEASED
  -> FIELD-QUALIFIED
```

No step may be skipped by assertion.

## Non-goals

Unless a blocker proves otherwise, do not add:

- NetBox projection;
- VMware import/migration;
- another VMM;
- another storage backend;
- broad OpenStack product claims;
- Kubernetes/operator machinery;
- a replacement database;
- a second CellHV runtime daemon;
- major UI redesign;
- speculative HA control-plane machinery.

## Final deliverable

Return a production-candidate report tied to exact commit and release identifiers containing:

- supported capability matrix and maturity state;
- real-host topology and test evidence;
- failures discovered and fixes applied;
- security/reproducibility checks;
- package/release verification;
- reference-deployment evidence;
- explicit unsupported behavior;
- residual risks that prevent a stable-production claim, if any.

If any required claim lacks the necessary evidence, leave it at the lower maturity state and report the missing proof instead of weakening the gate.
