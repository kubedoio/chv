# Phase B Native Listener Evidence

Date: 2026-07-21

Original slice scope: Unix HTTP transport lifecycle around the existing Core
router, initially unwired. Commit `e4448a6c` subsequently wired the listener
into the explicit, default-off `core-native` path. Default `legacy` startup and
VM runtime behavior remain unchanged.

Machine tests prove:

- an HTTP request is served over the Unix socket and the owned socket is 0600;
- injected unsupported-`fchmodat2` behavior exercises the pathname fallback
  and exact original-inode revalidation without depending on the host kernel;
- a malformed/truncated client is reaped without stopping later requests;
- concurrent bind and a pre-existing foreign file fail without replacement;
- shutdown stops acceptance and waits for an in-flight response;
- a foreign socket replacing the owned inode is not removed during cleanup;
- explicit shutdown propagates a listener-task panic;
- owner drop aborts a blocked handler and removes the owned socket;
- drain timeout aborts remaining connections and returns a structured error;
- successful shutdown removes only the originally owned socket.

Focused verification:

```text
cargo test -p cellhv-core-api
cargo clippy -p cellhv-core-api --all-targets -- -D warnings
cargo fmt --all -- --check
```

The `core-native` process harness subsequently proves production-binary listener
startup, bounded signal drain, owned-socket cleanup, stale owned-socket restart,
and refusal of a second authority. It does not prove a default-mode listener,
legacy/native convergence, VM lifecycle, process re-adoption, real KVM,
libvirt, or cloud compatibility.
