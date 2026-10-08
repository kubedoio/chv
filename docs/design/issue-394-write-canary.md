# Issue #394 — Concurrent-write storage migration: write canary (Option A, adopted)

Adoption record. Ruling 2026-10-08 (session, recorded on the issue):
**DP1 adopted** (state correction — the loss is loud-but-late post-#392,
not silent); **DP2 adopted** — Option A (write canary, fail fast) lands
now; **DP3 deferred** — Option C (pause-first opt-in mode) is a tight
follow-up, its own PR; **DP4 adopted** — Options B/D parked with
recorded triggers; **DP5** — the M4.6 recorded boundary stays as-is
under A.

The options write-up with the full analysis lives on the issue
(2026-10-08, `#394` comment "options write-up — request for a design
ruling"). This document records what landed and the boundaries that
hold after landing.

## 0. The state correction (DP1)

The issue was filed 2026-10-02 against a pre-#392 sender and titled the
defect *silent* data loss. The #392 finalize verification (merged
2026-10-03) changed the outcome: the sender computes a full-volume
SHA-256 over the source **post-pause**, the receiver re-computes it over
the assembled destination, and any divergence fails the task
`data_loss` with the VM best-effort resumed. Concurrent-write migration
is therefore **loud, not silent** — but **late**: a full transfer plus
two O(volume) digest passes to discover what a write canary can report
in microseconds. That lateness is the surviving defect Option A closes.

The three recorded facts the design rests on:

- **R1** — the dirty bitmap is stord-private userspace state, marked
  only by `LocalFileBackend::write_block`, whose only product caller is
  the migration *receiver* (on the destination volume). The plan §2.2
  write interception (io_uring / handle wrapping) was never implemented.
  On a source volume the bitmap is always zero; the dirty rounds are
  protocol machinery, not live-migration support.
- **R2** — there is no post-pause final sweep: after the pause the
  sender sends `FinalSync` and finalizes without re-reading anything.
  (Option B's subject, not A's.)
- **R3** — the practical outcome pre-A: fail late, maximally wasteful.

## 1. What landed (Option A)

**Mechanism.** `StorageBackend::write_canary_probe(volume_id, handle)`
returns a `WriteCanaryFingerprint` — the backing store's
`(mtime, ctime, size)` plus its `WriteCanaryCapability`:

- `FileStat` — regular-file backing (the local backend, implemented):
  every `write(2)` through **any** descriptor — Cloud Hypervisor's
  included — updates mtime/ctime, so the stat triple is a reliable
  tripwire for *whether* the source was written. `atime` is
  deliberately not sampled (the sender's own `read_block` reads would
  trip it on `relatime` mounts).
- `Unavailable` — the default trait implementation: block devices
  (LVM, iSCSI — data writes do not update the device node's
  timestamps) and RADOS objects (Ceph — no local path). The `Box<dyn
  StorageBackend>` blanket impl delegates, so a boxed local backend
  keeps its canary.

The sender samples the baseline immediately before bulk copy and
re-checks it **at every dirty-round boundary and at the pre-pause
gate**. A change fails the migration with `failed_precondition`
carrying the distinct `source_modified_during_migration` token —
*before* the VM is paused (no resume needed) and *before* the finalize
digest is computed (the task's `finalize_volume_digest` stays empty:
the observable early-vs-late discriminator). A probe error fails
closed (`internal`), never silently disarming the canary. On
`Unavailable` the sender proceeds exactly as before and logs the
reason: fail-late at the digest, the pre-A behavior.

## 2. The layering contract (what the canary is and is not)

The canary is a **latency-of-failure optimization**, not a correctness
gate:

- It claims *whether* the source moved, never *where* — placement
  truth stays with the #392 whole-volume finalize digest.
- Its last check is the **pre-pause gate**; writes landing between the
  gate and the pause completing are beyond its reach **by design** and
  remain fail-closed via the post-pause digest. (Pinned end-to-end:
  `source_write_during_pause_window_fails_at_finalize_digest`.)
- Symmetrically, writes landing **before the baseline** (between the
  trigger and the MigrationReady handshake) are inside not-yet-read
  blocks: bulk copy transfers them, nothing diverges, and the
  migration legitimately converges. The canary's baseline is sampled
  before the first source read, so the covered window is exactly the
  transfer window.
- It does not make live migration supported. The quiescent-only
  contract is unchanged; a canary failure tells the operator to retry
  with the source quiesced (or wait for Option C).

## 3. Test surface

- `migration_e2e.rs::concurrent_source_write_fails_fast` — an
  out-of-band in-place write during bulk copy (the guest-shaped path
  the bitmap cannot see) fails `failed_precondition` with the token,
  `needs_vm_pause` never set, `finalize_volume_digest` empty.
- `migration_e2e.rs::source_write_during_pause_window_fails_at_finalize_digest`
  — the §2 layering: a same-window write fails `data_loss` at
  finalize, digest non-empty.
- `migration_e2e.rs::dirty_rounds_converge_preseeded_writes` — the
  rewritten round-protocol test (pre-seeded dirty blocks, written
  *before* the baseline). The former
  `dirty_rounds_transfer_concurrent_writes` exercised a
  production-impossible shape — a stord-visible source write during
  migration — which is now, by design, a canary failure.
- Sender unit tests: no-op when unavailable; pass when unchanged; trip
  on mtime and on size-only movement; task marked Failed with the
  token.
- Local-backend unit tests: `FileStat` reported; an out-of-band
  in-place write trips; re-probe stable; foreign handles rejected
  fail-closed.

## 4. Parked with intent (DP4)

- **Option B — post-pause diff sweep** (true concurrent-write
  correctness up to the pause point, O(volume) pause window): reopen
  when a tenant-facing live-migration requirement lands.
- **Option C — pause-first opt-in mode** (stop-the-world, correct by
  construction): ruled a tight follow-up PR to this issue — it closes
  #394.
- **Option D — dm-snapshot COW** (true live migration, bounded
  pause): reopen when bounded-downtime live migration is required;
  device-backed volumes only.
