# CellHV Core duplicate-process proof blockers

## Current decision

The unwired Linux ownership observer cannot prove that a VM has exactly one
Cloud Hypervisor process. It returns `DuplicateEvidence::Indeterminate` and
the classifier returns `AmbiguousPreserve`. This is a deliberate recovery
blocker, not an implementation omission that may be replaced by a negative
`/proc` scan.

`DuplicateEvidence::Conflict` means a stable second owner was positively
observed. `DuplicateEvidence::Exclusive` means exclusivity was positively
established. No candidate being observed is not equivalent to `Exclusive`.

## Why `/proc` is insufficient

Linux `/proc` directory enumeration is not an atomic process snapshot. A
process can fork or exec after its numeric entry has been passed, between two
complete scans, or immediately after the final scan. PID reuse requires a
pidfd plus stable start time, boot ID, executable identity, credentials, and
cgroup evidence for each observed candidate, but those checks only establish
the identity of a process that was found. They do not establish global
absence.

Bounds are mandatory for recovery: entry count, bytes read, argument count,
and elapsed time must be capped. Reaching any bound is ambiguity. `hidepid`,
permission failures, disappearing entries, malformed command lines, and
unsupported pidfds are also ambiguity. Repeating the scan does not create a
linearization point and therefore cannot turn absence into exclusivity.

A future bounded scanner may safely improve diagnosis by returning
`Conflict` for a stable second Cloud Hypervisor process. It must return
`Indeterminate`, never `Exclusive`, when no conflict is observed.

## Prerequisites for positive proof

Positive proof requires an accepted supervision design before production
wiring. At minimum that design must define:

- one durable runtime generation mapped to one uniquely named systemd unit;
- PID 1 as the only legitimate launcher and lifecycle supervisor;
- a non-delegated per-VM-generation cgroup with verified unit, control-group,
  main-PID, and membership identity;
- atomic launch exclusion and durable correlation with the operation attempt;
- before/after revalidation around socket peer credentials and the Cloud
  Hypervisor API probe;
- the trust boundary for same-UID and privileged processes.

Systemd `MainPID` alone is insufficient because it says nothing about an
unregistered process. A cgroup membership scan alone still races membership
changes unless the supervision protocol supplies a kernel-backed stable
observation point. If arbitrary same-UID or privileged processes are in the
threat model, systemd/cgroups do not prevent a process outside the unit from
launching Cloud Hypervisor; stronger policy such as MAC confinement is needed.

The supervision protocol, trust boundary, unit naming, upgrade behavior, and
failure recovery semantics require an ADR. Until it is accepted and tested,
the Linux implementation remains `Indeterminate` and production VM launch and
management behavior remains unchanged.
