# CellHV Core Native Runtime Owner

Status: Phase B native-only library slice; not wired into `cmd/chv-agent`.

`cellhv-core-runtime-owner::CoreRuntimeOwner` is the bounded composition root
for a fresh native Core database and its later native restart. It consumes an
`ActivatedStore` and refuses activation when the validated provenance contains
a live NodeCache, any durable migration-state row regardless of source, a
NodeCache migration checksum, or an imported activation kind. This fail-closed
restriction remains until process-wide NodeCache facade authorization is
implemented.

For an eligible store the owner starts exactly one `AuthorityActor`, passes a
clone of that actor's bounded `AuthorityHandle` to exactly one
`CoreApiListener`, and retains the opaque `RuntimeAuthorityGuard`. It creates no
VM runtime, provider attachment, compatibility service, database, or operation
engine.

Explicit shutdown is ordered:

1. stop accepting and boundedly drain the native listener;
2. queue actor shutdown and join its database thread;
3. release the runtime authority guard.

Listener startup failure shuts down and joins the already-created actor before
returning and releasing the guard. It preserves the listener failure together
with every actor-cleanup failure. Actor startup failure releases the guard
without creating a listener. Explicit shutdown attempts every stage and returns
all observed failures.

Dropping the owner remains an emergency fail-closed path. It aborts the
listener and closes the actor owner, but the actor join is transferred to an
asynchronous reaper and cannot be observed from `Drop`. The runtime guard is
therefore deliberately forgotten, retaining its file descriptor and lease
until process exit. Replacement startup in the same process remains blocked;
production integration must await explicit shutdown to release the lease.

The public `OperationService` and lower-level actor/listener constructors remain
available for existing tests and libraries. Therefore this slice is positive
evidence for the `CoreRuntimeOwner` path only, not proof that production has no
other construction path.
