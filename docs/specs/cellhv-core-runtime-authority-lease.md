# CellHV Core Runtime Authority Lease

Status: Phase B library primitive; not wired into production.

## Purpose

`cellhv-core-fs::RuntimeAuthorityLease` is the process-lifetime exclusion
primitive for the future `chv-agent` Core authority. Exactly one process may
hold the lease derived from a Core database path. Acquisition is exclusive and
nonblocking: contention fails with `WouldBlock`, so startup never waits while
two processes appear healthy.

This lease is distinct from `AuthorityLock`. The latter serializes short
NodeCache save and migration-cutover transactions and is released after each
transaction. Production startup must retain the runtime lease for the entire
lifetime of the Core authority and must still use the transaction lock when
coordinating NodeCache cutover.

## Filesystem Contract

The lease is the sibling
`.${database-name}.cellhv-runtime-authority.lease`. Its immediate parent must be
a real, effective-user-owned `0700` directory. The lease must be a regular,
effective-user-owned `0600` file with exactly one link. `O_NOFOLLOW` prevents a
symlink target from being opened. Validation occurs both before and after open.
After acquiring `flock`, the implementation re-reads the pathname without
following symlinks and requires its device and inode to equal the locked file
descriptor. A pathname removed or replaced during acquisition fails closed.

`paths_alias` detects normalized lexical aliases and existing same-inode
aliases. Future startup wiring must use it to reject collisions between the
lease, database, SQLite sidecars, NodeCache, archive, sockets, and other
authority paths before acquiring or opening them.

The kernel releases `flock` when the descriptor or process exits. The file may
remain and is deliberately reusable; presence alone is not evidence of a live
authority. The primitive writes no PID because PID files are not ownership
proof and PID reuse makes them unsafe as the exclusion mechanism.

The lease pathname is persistent state. Normal package removal, service stop,
and package lifecycle cleanup must neither unlink it nor match it with a cleanup
glob. Stale presence is harmless because liveness is determined only by
nonblocking `flock`, and retaining the inode avoids creating two independent
lock namespaces during routine lifecycle operations.

## Trust Boundary

This protocol assumes cooperative processes running under the service UID and
an owner-only parent directory. It prevents accidental concurrent authorities
and rejects pathname replacement observed during acquisition. It does not
protect against a malicious or compromised same-UID process that can rename or
unlink the pathname after acquisition returns. Protecting against that actor
requires a stronger privilege boundary or directory ownership model; advisory
`flock`, ownership bits, and repeated pathname checks cannot provide it.

## Non-Claims

The library does not start an authority, open a database, bind an API, inspect
VMs, or alter Cloud Hypervisor behavior. `cmd/chv-agent` does not use it yet.
Production wiring must acquire it before opening the Core database and retain
the returned value until all authority actors and API listeners have stopped.
The tests do not claim hostile same-UID exclusion, distributed locking, or
durability across replacement of the containing filesystem.
