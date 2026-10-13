# Issue #457: Cloud Hypervisor `--no-shutdown` lifecycle

This changes VM lifecycle authority: a successful graceful stop now means the
guest is terminal while its VMM remains alive; delete explicitly shuts down
that VMM. Unexpected VMM exits still follow the persisted-config recovery
path, and force stop retains identity-checked SIGKILL plus process-death
verification.

This PR adopts the candidate that issue #457 records as previously declined
under the D6 option-(b) decision in issue #448. It therefore reopens that
lifecycle decision; the flag must not be treated as a dependency-only or
low-risk change. The implementation now accounts for the listed lifecycle
couplings: start boots a `Created` guest in place and advances the watchdog
console watermark; the console broadcaster reconnect is attempted after
`/vm.boot` recreates Cloud Hypervisor's serial endpoint; the vsock relay stays
bound across a guest stop; crashes and agent restarts retain the existing
respawn/adoption paths; and delete requests `/vmm.shutdown` before the
identity-checked kill fallback. The focused console regression test exercises
the endpoint being absent until `/vm.boot`.

Cloud Hypervisor v53.0's guest-exit path calls `vm_shutdown`, which takes and
shuts down the VM object while leaving the VMM control loop available under
`--no-shutdown` ([upstream v53.0 source](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/v53.0/vmm/src/lib.rs#L2095-L2104), [shutdown/boot handlers](https://github.com/cloud-hypervisor/cloud-hypervisor/blob/v53.0/vmm/src/lib.rs#L2224-L2248)). This releases guest-owned resources, but this PR has not re-run the M4.5/M4.6 storage and migration qualifications. The updated M4.3 real-KVM lifecycle qualification also remains unrun, so same-process stop/start and the full integration lifecycle still need runtime evidence before the lifecycle change can be considered fully qualified.

The regression tests in
[`process.rs`](../../crates/chv-agent-runtime-ch/src/process.rs) cover a live
VMM and vsock relay surviving guest shutdown, a later `/vm.boot` reusing that
same process with a fresh watchdog watermark and a re-established console
broadcaster, and delete requesting `/vmm.shutdown`. The real-KVM lifecycle qualification in
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
