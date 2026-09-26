# Prompt 06 — Reference Deployment and Field Qualification

Run the published RC as a real bounded deployment and convert release evidence into operational evidence.

## Preconditions

- Prompt 05 produced a published RC.
- Use only the published candidate artifacts for the deployment.
- The deployment topology and support scope are written before the run.
- Define which failures are acceptable limitations and which terminate field qualification.

## Goal

Prove that CHV can operate its first supported use case over time, not only pass an installation test.

The objective is not to demonstrate every repository feature. It is to demonstrate one coherent product profile that an operator could realistically adopt.

## Reference profile

Choose and record one narrow profile, for example:

- one control plane;
- two or three KVM/Cloud Hypervisor nodes;
- one qualified network profile;
- local/LVM plus at most one qualified shared-storage profile;
- existing Web UI/CLI/API;
- backup target if backup/restore is part of the support claim.

Do not broaden the profile during the campaign merely because another feature exists in code.

## Required work

### 1. Deploy from released artifacts

Recreate the environment from clean hosts using only published packages and documentation.

Capture:

- hardware/CPU/KVM capabilities;
- OS/kernel;
- Cloud Hypervisor/firmware versions;
- CHV package versions;
- topology;
- storage/network dependencies;
- redacted configuration.

### 2. Operate normal workload

Run a representative bounded workload long enough to observe steady state.

Exercise:

- repeated VM create/delete;
- start/stop/reboot;
- console/normal operator visibility;
- network policy changes;
- storage attach/use;
- migration where supported;
- backup and restore where supported.

### 3. Planned failure/recovery drills

Execute and document:

- control-plane restart;
- control-plane outage;
- agent restart with running VMs;
- compute-host reboot;
- provider daemon restart;
- network disruption/reconnect;
- migration interruption;
- backup-target outage;
- certificate/configuration failure cases that are safe to reproduce.

Verify running-workload and new-mutation behavior matches the published partition/security policy.

### 4. Upgrade drill

When a successor RC/build is available during the field campaign, use the supported rolling upgrade path.

Otherwise rerun the RC package lifecycle/upgrade simulation in the reference topology.

Capture:

- drain behavior;
- workload preservation/migration;
- component version skew;
- health gates;
- failure rollback/recovery;
- return to schedulable state.

### 5. Observability and operator usability

Determine whether an operator can answer from supported tools:

- which VMs are running where;
- node health and readiness;
- failed/stuck operation and reason;
- migration progress;
- storage/network failure;
- certificate/enrollment failure;
- resource pressure;
- what action is safe next.

Record missing metrics/logs/runbooks as operational defects.

### 6. Resource/leak checks

Across the campaign check for unexplained growth or residue in:

- daemon RSS/CPU;
- SQLite/database size;
- open files/sockets;
- Cloud Hypervisor processes;
- TAP/veth/bridge/VXLAN objects;
- nftables objects;
- local volume/session state;
- temporary backup/migration artifacts.

### 7. Final support decision document

For each major capability record:

- maturity state;
- exact evidence;
- known limitation;
- operator workaround if one exists;
- whether it is allowed in the stable-production profile.

Do not turn an unresolved defect into a documentation-only workaround when it threatens isolation, data integrity, authority, or recovery.

## Acceptance criteria

- deployment can be reproduced from release artifacts and docs;
- normal workload remains stable for the defined field window;
- planned restarts/outages recover according to contract;
- no unexplained resource or ownership leaks remain;
- backup/restore and migration claims have field evidence if included;
- operator diagnostics are sufficient for the supported profile;
- support matrix is narrower than or equal to the evidence;
- blocker-class residual risks are zero before a stable-production recommendation.

## Forbidden outcomes

- using source builds or unreviewed local patches in the final field run;
- broadening the support matrix from code presence alone;
- masking isolation/data-loss/authority defects as known limitations;
- requiring hidden maintainer knowledge for ordinary recovery;
- claiming HA or scale characteristics not tested by the reference topology.

## Exit gate

Prompt 06 passes when the released RC has operated a reproducible reference deployment with documented recovery and no unresolved blocker-class defects. Only then may the project evaluate whether that exact support profile is ready to move from technical preview/RC toward stable production.
