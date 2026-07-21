# Phase B Runtime Authority Lease Evidence

Date: 2026-07-21

Original slice scope: library-only process-lifetime authority exclusion. At the
time of this slice, production `chv-agent` and Cloud Hypervisor behavior were
unchanged.

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

Commit `e4448a6c` subsequently made `cmd/chv-agent` acquire and retain this lease
for the explicit `core-native` process lifetime. Process tests prove a second
native instance is refused and that clean or killed-process restart can
reacquire the lease. Omitted/default `legacy` mode does not use the Core lease,
and native mode refuses legacy NodeCache state, so this is not yet proof that
old and new request paths share one authority.

This is not Cloud Hypervisor process recovery, host-reboot recovery, real-KVM,
libvirt, or platform qualification evidence.
The evidence assumes cooperative service-UID processes in an owner-only
directory. It does not prove exclusion against a malicious same-UID process
that replaces the pathname after acquisition has returned.
