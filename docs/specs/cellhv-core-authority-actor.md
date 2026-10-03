# CellHV Core Authority Actor

Status: Production wiring (core-managed and core-native authority modes). Every production path that opens Core state composes the actor; the legacy NodeCache path does not.

## Boundary

`cellhv-core-operations::AuthorityActor` is the asynchronous serialization
boundary around the existing `OperationService`. It is not another operation
engine: the actor owns exactly one `OperationService`, and only that service
accesses `cellhv-core-store`.

The actor accepts an already opened and startup-validated service. It cannot
create a database, select NodeCache versus Core authority, execute a VM action,
contact `chv-stord` or `chv-nwd`, or open a Cloud Hypervisor API socket.
`cmd/chv-agent` constructs it once through `CoreRuntimeOwner` in the
core-managed and core-native authority modes; `AgentServer` and the native API
receive clones of its handle.

## Queue Contract

The queue has an explicit nonzero capacity. `send().await` provides
backpressure; requests are never silently dropped. Mutation and inspection
requests are processed in queue order by one named OS thread, so synchronous
SQLite work never blocks a Tokio runtime worker.

Once enqueue succeeds, cancelling the caller does not cancel the authority
request. The actor may commit after the reply receiver disappears. A caller
that loses its reply must retry with the identical scope, idempotency key, and
request so the operation journal resolves the ambiguity.

Shutdown is an explicit queue message. Requests ordered before it complete.
When it is processed, the receiver closes, the shutdown acknowledgement is
sent, and requests ordered later either fail to enqueue or lose their reply with
`Unavailable`. Successful enqueue therefore does not promise execution when a
shutdown message is ahead of that request. Explicit `join()` closes and drains
the bounded channel and joins the OS thread through Tokio's blocking pool.
Dropping the owner closes the channel and transfers the join handle to a named
reaper thread, so implicit cleanup cannot block a Tokio worker and a surviving
handle cannot retain an unowned authority worker. Production wiring must use
explicit shutdown and `join()` so thread failure remains observable.

## Exposed Operations

The actor exposes durable mutation submission, the destructive-recovery
capabilities — `ExecutionHandle::classify_restart_interrupted` and
`mark_operation_abandoned` on the executor domain, and
`AuthorityHandle::resolve_inspect_required` on the handle surface (the
runtime owner additionally calls the service-level
`OperationService::classify_restart_interrupted_operations` once at startup,
before the actor is spawned) — each gated by the store's marker and token
fences — and read-only host, VM, operation, event, and restart inspection. It
intentionally does not expose
`claim_attempt` or `finish`: those belong to a later bounded executor slice and
would create an accidental runtime execution boundary here.

The native router is a pure transport constructor over an injected
`AuthorityHandle`. It does not accept `OperationService`, open a store, or
start a private actor. Handler reads and mutations therefore enter the same
bounded, queue-ordered authority surface intended for compatibility adapters.

## Production wiring

Production construction occurs once in `chv-agent` after
`cellhv-core-startup` selects Core authority, while holding an authority lease
for the actor lifetime. Legacy gRPC and the native local API receive clones of
the same handle. `OperationService` nevertheless remains publicly
constructible for library and test surfaces, so multiple independently
constructed actors remain possible at the library API level. Production
composition must close that residual before claiming process-wide exclusivity
or `AGENT-CORE-002` completion.
The former private `cellhv-core-api::DbActor` has been retired; production
wiring injects a clone of the one process-wide handle and retains its
`AuthorityActorJoin` owner until ordered shutdown completes.
