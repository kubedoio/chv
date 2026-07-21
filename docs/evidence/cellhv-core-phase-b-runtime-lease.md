# Phase B Runtime Authority Lease Evidence

Date: 2026-07-21

Scope: library-only process-lifetime authority exclusion. Production
`chv-agent` and Cloud Hypervisor behavior are unchanged.

## Machine Evidence

- A subprocess cannot acquire the lease while the parent process holds it;
  acquisition returns `WouldBlock` rather than waiting. A marker written only
  by the selected child helper prevents a zero-test subprocess from passing.
- A second independently opened descriptor in the same process also receives
  `WouldBlock` while the first lease is live.
- After the owning subprocess exits, the persistent lease file is reusable,
  proving that stale file presence does not block startup.
- Negative tests reject symlink and hard-linked lease files and unsafe modes,
  and exercise normalized-path and inode alias detection.
- A deterministic acquisition hook replaces the pathname after `flock`; the
  post-lock device/inode comparison detects the replacement and fails closed.
- The lease uses an owner-owned `0700` parent, `O_NOFOLLOW | O_CLOEXEC`, an
  owner-owned `0600` regular file with one link, and `LOCK_EX | LOCK_NB`.
- Architecture tests scan package scripts, systemd units, and the authoritative
  install/development cleanup scripts. They reject explicit lease removal,
  lease globs, recursive removal of the authority parent or state root, hidden
  contents removal, and `find ... -delete` over that namespace while allowing
  non-destructive directory creation and permission setup.

Focused verification:

```text
cargo test -p cellhv-core-fs
cargo clippy -p cellhv-core-fs --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Non-Claims

This is not production startup, crash-recovery, real-KVM, libvirt, or platform
qualification evidence. The runtime lease is not yet constructed by
`cmd/chv-agent`.
The evidence assumes cooperative service-UID processes in an owner-only
directory. It does not prove exclusion against a malicious same-UID process
that replaces the pathname after acquisition has returned.
