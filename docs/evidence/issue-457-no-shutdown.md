# Issue #457: Cloud Hypervisor `--no-shutdown` lifecycle

This changes VM lifecycle authority: a successful graceful stop now means the
guest is terminal while its VMM remains alive; delete explicitly shuts down
that VMM. Unexpected VMM exits still follow the persisted-config recovery
path, and force stop retains identity-checked SIGKILL plus process-death
verification.

The regression tests in
[`process.rs`](../../crates/chv-agent-runtime-ch/src/process.rs) cover a live
VMM and vsock relay surviving guest shutdown, a later `/vm.boot` reusing that
same process with a fresh watchdog watermark, and delete requesting
`/vmm.shutdown`. The real-KVM lifecycle qualification in
[`m4.3-lifecycle.sh`](../../scripts/integration/qual/m4.3-lifecycle.sh) now
checks the same-process stop/start contract; this qualification run has not
been performed for this change yet.

Local verification:

- The three issue-specific lifecycle tests pass.
- `cargo clippy -p chv-agent-runtime-ch --all-targets -- -D warnings` passes.
- `cargo fmt --all` and `bash -n scripts/integration/qual/m4.3-lifecycle.sh` pass.
- The full crate test run passes 110 of 123 tests. The 13 failures are in
  pre-existing secure-path/adoption tests and return `UnsafePath` when their
  temporary fixtures are created under the group-writable workspace; no
  issue-specific test fails.
