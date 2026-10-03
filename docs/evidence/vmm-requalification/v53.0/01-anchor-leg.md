# D6-(b) Leg 01 — Anchor: kvm-smoke matrix on Cloud Hypervisor v53.0

> Campaign: [#448](https://github.com/kubedoio/chv/issues/448) (D6, option (b)) · [Campaign index](README.md)
> Date: 2026-10-03 (candidate runs 20:00–20:02 UTC; fixed-harness rerun same day)
> Execution: subagent, execution + reporting only (no repo changes during the leg; the harness defect found was fixed by a separate reviewed PR, #459)
> Host: the qualification host (m4.1-class after resize; 16 vCPU AMD EPYC, 31 GiB RAM, `/dev/kvm`, kernel `6.8.0-142-generic`, Ubuntu 24.04.5)

## Verdict

**PASS at harness tier.** The v53.0 candidate is downloadable, digest-stable,
executable on this host/kernel, and passes the kvm-smoke deployment/health
matrix on the fixed harness — with the qualified v43.0 system pin provably
untouched throughout. **This leg makes no guest-facing claim**: kvm-smoke never
boots a VM (its lifecycle section is a TODO stub, `kvm-smoke.sh:695-711`), so
nothing here exercises the serial-console chain, poweroff/exit timing, or any
booted-guest behavior. Those live in the subsequent legs.

## 1. Execution model and pin-safety proof

The hypothesized isolation model ("kvm-smoke downloads its own pinned copy when
`CHV_CLOUD_HYPERVISOR_VERSION` is set") was **false on this host at the time of
the leg**: the script consulted the override only when
`/usr/bin/cloud-hypervisor` was missing, and its download branch wrote directly
to `/usr/bin/cloud-hypervisor` — as did `qual/env-preflight.sh:117-136` on
version mismatch. With the qualified v43.0.0 pin installed, a bare
`CHV_CLOUD_HYPERVISOR_VERSION=v53.0` invocation was inert (it silently re-ran
v43.0).

Model used instead: the v53.0 candidate staged in the campaign workdir and
kvm-smoke executed inside a **private mount namespace** (`unshare --mount`)
with the candidate bind-mounted over `/usr/bin/cloud-hypervisor`
namespace-locally. The mount vanished when the run exited; the system file was
never opened for write.

| | sha256 of `/usr/bin/cloud-hypervisor` | `--version` |
|---|---|---|
| host, before runs | `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496` | v43.0.0 |
| inside namespace (pre-run check) | `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc` | v53.0 |
| host, after both runs | `a250a9347d0ea9e93f88b54b25df3cdc6a9ba3c57f292aaf74bb664fb5c87496` | v43.0.0 |

No `cloud-hypervisor` entry in host `/proc/self/mountinfo` after the runs.

## 2. Initial result — FAILED, and why that was not a v53.0 regression

Invocation: `CHV_CLOUD_HYPERVISOR_VERSION=v53.0 CHV_TEST_TIMEOUT=30
./scripts/integration/kvm-smoke.sh --binary-dir /var/lib/chv/qual/bin` inside
the namespace, with the staged candidate CHV binaries (`af2dfdcd`, 0.2.0,
build 2026-10-03). A **v43.0 control run** (system pin, no namespace) used the
same command.

Result on both: **FAILED — 1 error, 6 warnings**, identical byte-for-byte.
Failing assertion: `chv-agent is not running`; agent log:
`Error: UnsafePath("…/agent must be an owner-owned 0700 or 0750 directory")`.
Everything else passed (KVM present, binary/version checks ×5, dev-environment
generation, controlplane `:8080`, stord, nwd; 4 expected systemd-unit warnings
in binary-dir mode; agent metrics/socket warnings were downstream of the agent
death).

Classification — **pre-existing harness defect on post-#334 main, version-independent**:

- kvm-smoke's generated `agent.toml` sets `authority_mode = "core-managed"`
  (flipped by #334, merged 2026-10-01) but `generate_dev_environment` never
  chmods the agent runtime dir — `mkdir -p` subdirs get 0755 under umask 022,
  and the fail-closed Core-authority startup validation
  (`cellhv-core-startup::validate_paths`, from #304) rejects 0755. The agent
  exited at startup, **before any VMM interaction**.
- The requirement is documented in frozen m2.5 §4 (the m2.5 harness mirrors it
  with `chmod 0700`). kvm-smoke's legacy-mode exemption note was true when
  written; #334 flipped the config without adding the chmod.
- The last passing kvm-smoke run on this host (Sep 30, legacy mode) predated
  #334. **No kvm-smoke run had executed on post-#334 main until this leg** —
  the `kvm-test` PR label gates the workflow, and no PR had carried it.
- The v43.0 control reproduced the failure identically.

Per the leg's stop-don't-improvise instruction, the failure was captured and
the run stopped; no workaround was attempted (e.g. a 0077 umask inside the
namespace would have masked the defect and produced a run unrepresentative of
CI).

## 3. Resolution and fixed-harness rerun — PASS

The defect was filed as **#458** and fixed by **#459** (merged 2026-10-03;
independent review verdict READY with live reproduction on this host). The fix
mirrors `qual/deploy.sh`: `chmod 0700` on the agent runtime dir **and** the
core-managed paths (`core_store_path`/`core_archive_path`/`core_api_socket_path`)
pinned into the generated config — a second gap found live during
verification. #459 also made the version override real and pin-safe: an
explicit `CHV_CLOUD_HYPERVISOR_VERSION`/`--chv-version` is now honored
unconditionally and stages the requested version in a private temp dir;
`/usr/bin/cloud-hypervisor` is never written when an override is requested.

Fixed-harness rerun on this host (evidence in #459, independently reproduced
by its reviewer):

| Run | VMM | Result |
|---|---|---|
| #459 verification, system binary, no override | v43.0.0 (system pin) | `[RESULT] PASSED with 4 warning(s)` |
| #459 verification, `CHV_CLOUD_HYPERVISOR_VERSION=v53.0` | v53.0 (staged) | `[RESULT] PASSED with 4 warning(s)` — log: system v43.0.0 detected, v53.0 requested, staged binary used |
| #459 reviewer reproduction, `--chv-version v43.0` | v43.0.0 (system, no download) | `[RESULT] PASSED with 4 warning(s)` |

System pin sha256 `a250a934…` verified unchanged before and after every run.
The 4 warnings are the expected systemd-unit ones in binary-dir mode.

## 4. Candidate provenance (the tuple this campaign pins)

- URL: `https://github.com/cloud-hypervisor/cloud-hypervisor/releases/download/v53.0/cloud-hypervisor-static`
  (asset naming identical to v43.0; `ch-remote-static` also verified downloadable)
- `cloud-hypervisor-static` v53.0: sha256
  `448af3d4e59b22c2987f7df94c213ad40fb53a10d437e42b5ee6c4fce7c29ecc`,
  7,062,256 bytes, `--version` → `cloud-hypervisor v53.0` /
  `Migration Protocol Versions: 0`
- `ch-remote-static` v53.0: sha256
  `13f32ba952e6791fd901f2279be2055fbacc64005f96c42a8e90d58860df84a7`,
  `--version` → `ch-remote v53.0`
- Both executed successfully on kernel `6.8.0-142-generic` (amd64). Binaries
  were deleted after the leg (duration-only isolation); subsequent legs
  re-download and pin by these digests.

## 5. Structural scope limits

kvm-smoke never creates a VM (lifecycle TODO stub). The candidate binary
executed only for `--version` and as the configured `chv_binary_path` during
unit health checks. Therefore **no observation was possible** on: the
serial-console chain (Pty gate / socket fd leak / pre-connect buffering /
silent serial-manager thread death), guest poweroff → VMM exit timing (#345
class, including v53's `EpollDispatch::GuestExit` refactor and the
not-adopted `--no-shutdown` flag per campaign decision Option B), or anything
else requiring a booted guest. The lifecycle matrix lives in
`scripts/integration/qual/*`, which hardcodes `/usr/bin/cloud-hypervisor` the
same way — the mount-namespace model generalizes to it unchanged, and is the
campaign-standard isolation adopted in the [campaign index](README.md).

## 6. Teardown and host state

Verified clean after all runs: no `/tmp/chv-kvm-test-*` dirs, no chv-* or
cloud-hypervisor processes, no test bridges/taps, no listeners on
:8080/:8443/:9100, no bind mounts in the host namespace, disk unchanged
(~71 GiB free on /), repo working tree clean. One pre-existing side effect
observed (present in baseline, not caused by this leg): the controlplane's
first-boot logic wrote a DB backup under `/var/lib/chv/backups/` (documented
CP behavior on fresh DBs).

## 7. Follow-ups recorded by this leg

- **#458 → #459** (closed): the blocking harness defect and its fix.
- `qual/env-preflight.sh:117-136` still actively reinstalls over
  `/usr/bin/cloud-hypervisor` on version mismatch — an intentional
  version-enforcement gate for the frozen v43.0 campaign; disclosed in #459's
  residual risk. Any change there belongs to the pin-move PR, not mid-campaign.
- The `integration-kvm.yml` `test_level` input (basic/full) is decorative —
  both run the same command. Known limitation, recorded in #459.

## Artifacts

Campaign workdir (ephemeral): `kvm-smoke-v53.log`, `kvm-smoke-v43-control.log`,
`host-baseline.txt`, and the leg report; the load-bearing excerpts are
embedded above. The fixed-harness rerun evidence is preserved in #459's PR
record.
