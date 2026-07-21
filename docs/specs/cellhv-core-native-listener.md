# CellHV Core Native Unix HTTP Listener

Status: Phase B library slice; not wired into production startup.

`cellhv_core_api::CoreApiListener` is the sole owner of one native HTTP Unix
socket and its accept task. It serves the existing `router(AuthorityHandle)`;
it does not open Core state, create an `OperationService`, or introduce another
authority actor.

Startup uses `bind_private`, requiring an effective-user-owned real 0700 parent
and refusing every existing destination without unlinking it. After bind it
opens the pathname with `O_PATH|O_NOFOLLOW`, obtains the filesystem identity by
`fstat`, applies 0600 with `fchmodat2(AT_EMPTY_PATH)`, and verifies socket type,
device, inode, and mode through the pathname. Kernels lacking that operation
fall back, within the cooperative 0700 parent boundary, to pathname chmod and
then reopen with `O_PATH|O_NOFOLLOW` to revalidate the original identity and
mode. The AF_UNIX listener fd itself is
not used as pathname identity because Linux reports its socketfs inode. Every
cleanup compares the verified pathname identity first; a path unlinked and
replaced by another owner is preserved. The 0700 parent is a cooperative
same-UID trust boundary, not protection against a malicious peer with the same
effective UID.

`shutdown()` stops acceptance, continuously reaps tracked Hyper connections,
signals graceful shutdown, and waits for in-flight requests up to a configurable
timeout (30 seconds by default). Timeout aborts and reaps remaining connections
and returns `DrainTimeout`; accept errors, connection errors, connection-task
panics, and listener-task panics are also structured. Dropping the owner aborts
the listener task and its connection set on a best-effort asynchronous basis;
explicit bounded `shutdown()` remains mandatory for an observed, joined
shutdown result and for orderly authority release.

Production integration remains disabled. Startup lease/identity selection must
complete before creating this listener, and the process shutdown coordinator
must explicitly await `shutdown()` before releasing runtime authority.
