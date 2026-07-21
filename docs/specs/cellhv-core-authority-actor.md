# CellHV Core Authority Actor

Status: Phase B library slice; not wired into production.

## Boundary

`cellhv-core-operations::AuthorityActor` is the asynchronous serialization
boundary around the existing `OperationService`. It is not another operation
engine: the actor owns exactly one `OperationService`, and only that service
accesses `cellhv-core-store`.

The actor accepts an already opened and startup-validated service. It cannot
create a database, select NodeCache versus Core authority, execute a VM action,
contact `chv-stord` or `chv-nwd`, or open a Cloud Hypervisor API socket. Neither
`cmd/chv-agent`, `AgentServer`, `VmRuntime`, nor the native API constructs it in
this slice.

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

The actor exposes durable mutation submission and read-only host, VM,
operation, event, and restart inspection. It intentionally does not expose
`claim_attempt` or `finish`: those belong to a later bounded executor slice and
would create an accidental runtime execution boundary here.

## Required Future Wiring

Production construction must occur once in `chv-agent` after
`cellhv-core-startup` selects Core authority, while holding an authority lease
for the actor lifetime. Legacy gRPC and the native local API must receive clones
of the same handle. Direct transport construction of `OperationService` must
then be prevented.

Until that wiring exists, multiple independently constructed actors remain
possible at the library API level. Therefore this slice is not evidence of
process-wide exclusivity, production cutover, or `AGENT-CORE-002` completion.
The existing private `cellhv-core-api::DbActor` must be retired and the API must
receive this shared handle during that wiring; both actors must not remain as
independent production paths.
