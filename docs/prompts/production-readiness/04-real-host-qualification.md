# Prompt 04 — Real-Host and Multi-Host Qualification

Turn implemented capability into release evidence on real KVM hosts.

## Preconditions

- Prompts 01–03 are merged or the campaign declaration records why a gate is not applicable.
- The exact candidate SHA is fixed for the qualification run.
- Disposable real Linux/KVM hosts are available.
- Use released/package-equivalent service layout; do not rely on developer-only shortcuts that bypass systemd, permissions, certificates, or packaging behavior.

## Goal

Prove the exact candidate on real infrastructure across lifecycle, restart/recovery, network isolation, storage, migration, backup/restore, upgrade primitives, and basic performance/soak behavior.

Do not invent support claims for paths that are not exercised.

## Minimum topology

Prefer at least:

- one control-plane host;
- two KVM/Cloud Hypervisor compute hosts;
- the exact qualified network profile;
- the exact qualified storage profile(s);
- an external S3/NFS backup destination when backup is in candidate scope.

If the release claims more than this topology proves, either expand the lab or narrow the release claim.

## Required work

### 1. Clean installation baseline

From clean supported hosts:

- install the candidate packages or package-equivalent artifacts;
- validate service users, directories, permissions, sockets, config validation, and startup ordering;
- enroll compute nodes through the supported security path;
- capture component versions and exact configuration with secrets redacted.

### 2. Lifecycle and recovery

Run on real KVM:

- create;
- start;
- guest boot/readiness;
- reboot;
- stop;
- start again;
- delete;
- agent restart while VM runs;
- control-plane restart while VM runs;
- host reboot with documented expected recovery;
- management-plane outage and reconnect behavior.

Prove identity and operation history remain deterministic.

### 3. Network qualification

Execute Prompt 01's privileged host-safety suite against the candidate.

Also prove the exact advertised guest path:

- attachment;
- connectivity;
- policy allow/deny;
- detach;
- cleanup;
- restart/reconcile.

Record unsupported coexistence environments explicitly.

### 4. Storage qualification

For every storage backend claimed by the candidate, run the reusable contract:

```text
validate -> provision/consume -> attach -> guest write/read
-> restart/interruption -> recover -> detach -> cleanup -> repeat
```

For Ceph RBD or iSCSI, use a real external system before promoting to FIELD-QUALIFIED or broad production support.

Unit tests are not external-array/cluster qualification.

### 5. Storage migration mTLS qualification

Even if configuration wiring is already merged, prove the real two-stord path.

Required positive case:

- source stord and destination stord use identities issued by the test/deployment CA;
- migration completes over mTLS;
- dirty rounds and paused final sync execute;
- destination data is verified.

Required negative cases:

- missing TLS config;
- wrong CA;
- wrong destination identity/server name;
- mismatched keypair;
- malformed certificate/key/CA;
- expired certificate where the test framework can construct one;
- plaintext endpoint/downgrade attempt;
- interrupted transfer and deterministic retry/recovery.

Do not close the evidence gate because a configuration unit test passes.

### 6. Backup and restore qualification

If backup is in the release claim:

- create guest data with a known digest;
- execute the supported backup path;
- prove off-host artifact existence and checksum/metadata;
- simulate loss of the restorable local object according to the documented procedure;
- restore through the supported restore procedure, automated or manual;
- boot/read the restored workload and verify data digest;
- prove retention cleanup does not delete the wrong artifact.

If restore remains manual, say so explicitly; a backup without restore validation is not a complete DR claim.

### 7. Fault and interruption matrix

Exercise at least:

- service restart during active operation;
- source/destination migration interruption;
- control-plane loss;
- agent reconnect;
- storage/network provider restart where supported;
- repeated cleanup/idempotent retry.

Every failure test asserts forbidden outcomes, not merely eventual success.

### 8. Performance and soak baseline

Measure rather than guess.

Capture:

- idle memory/CPU by daemon;
- lifecycle operation latency;
- API/control-plane latency under a bounded concurrent workload;
- migration throughput for the qualified storage path;
- control-plane database growth during the run;
- open file/socket growth;
- log/metric cardinality;
- at least a bounded steady-state soak with repeated operations.

Do not create marketing-scale limits from a single lab. Record the hardware/topology and label results as baseline measurements.

## Evidence

Store durable evidence tied to exact SHA and environment under the campaign evidence root.

Include commands, versions, redacted config, test results, logs/artifact references, failures, retries, and cleanup results.

## Acceptance criteria

- exact candidate passes real-KVM lifecycle and recovery;
- network isolation gate passes on real Linux;
- every advertised storage path has the corresponding real-system evidence level;
- two-stord mTLS migration passes positive and negative identity cases;
- backup claim includes restore validation or is explicitly narrowed;
- interruption/retry does not duplicate or lose authoritative state;
- baseline performance/soak data exists without unsupported scale claims;
- cleanup leaves no unexplained VM, disk, network, process, or credential residue.

## Forbidden outcomes

- using mocks as real-host evidence;
- testing source builds while releasing different packages;
- silently skipping failed destructive tests;
- promoting untested Ceph/iSCSI paths because their unit tests pass;
- calling backup "DR" without restore evidence;
- inventing capacity limits not measured by the run.

## Exit gate

Prompt 04 passes when the exact candidate SHA has a reproducible, real-host evidence bundle for every capability the RC intends to claim.
