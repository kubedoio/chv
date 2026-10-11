# G4 part 2 — alerting and signed notifications on a real VM

**Campaign:** native monitoring implementation (#602)
**Gate:** G4 part 2 (alerting — prompt 05, tasks 1–6 + 8)
**PR:** PR-6 (`monitoring-g4b-alerting`)
**Date:** 2026-10-11 (UTC)
**Environment:** identical to the G0b/G1/G2/G3/G4 captures — AMD EPYC 9554P,
kernel `6.8.0-142-generic`, real KVM, `cloud-hypervisor v53.0` static
binary digest-verified against `scripts/install.sh`'s pin
(`448af3d4e59b22c2…`), rust-hypervisor-fw `4a0a1e97…`, guest image
`noble-qual-patched.img` `37f7c340…`. The guest package under test is
`chv-monitor-agent_0.3.0_amd64.deb` (`5bb9d89c255eae1a…`) built from the
final branch state (`e2847ffb`, all seven review rounds' fixes compiled
in; byte-compared against `target/release/chv-monitor-agent`
— `486667d6…` on both — before the run). The manager runs
in-process in the test binary — the real
`chv-controlplane-service` HTTPS/TLS/ingest stack, the real
`MonitoringStore` with the alerting DDL, the real evaluator and
dispatcher workers, and a real HTTPS webhook receiver on the host
bridge.

## Method

### 1. Full vertical on a real VM

The env-gated integration test `g4b_real_vm_alerting_and_signed_notifications`
(`cmd/chv-monitor-agent/tests/g4b_alerting.rs`) extends the G3/G4 rig's
production path (bridge + tap on `192.168.64.0/24`, real
`vm.create`/`vm.boot`, enriched NoCloud seed, `dpkg -i`, one-time claim,
mTLS enrollment) with the alerting surface: the alert rule is created
**before** the VM boots (the evaluator simply sees the condition not met
until the guest reports), a real `g4-http.service` in the guest backs the
`http:app` check, and a real HTTPS receiver on the bridge (a private-range
address — deliberately allowed, since operator config is the webhook
allowlist) captures every notification.

The scenarios (each a real guest transition driven over ssh, then
observed through the manager's own query paths — never by reading guest
state directly):

- **Healthy baseline** — the app check reports ok; **no incident, no
  webhook** (silence is not vacuous: the negative is asserted, not
  assumed).
- **Real outage** — `sudo systemctl stop g4-http.service`: the check
  goes critical through the real collectors, the hold window (15 s)
  elapses, and the evaluator opens a **firing** incident with the rule's
  severity and the VM as its resource.
- **Signed firing webhook** — exactly one firing webhook arrives at the
  receiver; `content-type: application/json`; the `x-chv-signature`
  verifies against an **independent HMAC-SHA256 computed by the rig**
  over the raw body (not the dispatcher's own signing path); the body is
  the contract envelope (event type, severity, target, incident id,
  event id, a summary that names the rule).
- **Delivery audit honesty** — the outbox row for the firing event is
  marked `delivered` with its attempt count; no dead letters in the
  healthy scenario.
- **Real recovery** — `sudo systemctl start g4-http.service`: the check
  recovers, the recovery window (15 s) elapses, and the incident
  **resolves** (with `resolved_at`), producing exactly one signed
  **resolved** webhook carrying the same incident identity.
- **Teardown** — graceful `stop_vm` + `delete_vm`, bridge and tap
  removed, no leaked VMM process.

```sh
CHV_G1_VMM_BINARY=/tmp/opencode/g0b/cloud-hypervisor \
CHV_G1_FIRMWARE=/var/lib/chv/qual/hypervisor-fw \
CHV_G1_IMAGE=/var/lib/chv/qual/images/noble-qual-patched.img \
CHV_G4_AGENT_DEB=$PWD/dist/packages/chv-monitor-agent_0.3.0_amd64.deb \
cargo test -p chv-monitor-agent --test g4b_alerting -- --nocapture
```

Result: **1 passed in 205.13s** (CI skips this test — no KVM; the run above is
the real-host record on the final branch state; verbatim observations
below).

### 2. In-process depth (identical manager/TLS code)

The evaluator, dispatcher, store and BFF behavior is pinned by the
workspace suites: 85 evaluator library tests (condition semantics, hold
and recovery windows, rate reset safety, gap/no-data policies,
acknowledge-while-firing, silence expiry, stale-open guard, orphan
sweep incl. the pending-delete branch and the disabled-rule
non-retirement), the dispatcher suite (crash between persistence and
send, duplicate webhooks, timeout/429/500/permanent-400, backoff,
dead-letter, redaction), the BFF route tests (revision 409s, retire on
delete/dimension-edit, cross-tenant, viewer tier), and the alerts e2e
specs (7) in the full Playwright run (66/66).

## Observed (verbatim from the recorded run)

```text
g4b checkpoint: network up (bridge + tap)
g4b checkpoint: rig up: ingest on https://192.168.64.1:40247, receiver at https://192.168.64.1:46647/hook
g4b checkpoint: alert rule created: f606b346-344d-4771-9682-f59f2fa0fb29 on g4b-vm (check http:app -> critical)
g4b checkpoint: creating vm (production adapter, seed built)
g4b checkpoint: seed enriched with the agent package and claim
g4b checkpoint: agent enrolled
g4b checkpoint: ssh reachable
g4b checkpoint: app check healthy (status ok, summary Some("HTTP 200"))
g4b checkpoint: g4-http.service stopped in the guest
g4b checkpoint: incident 0c1fdc9b-dc51-4edf-8d87-8063b4b75208 firing (opened at 2026-10-11T00:13:35Z, last observed Some("critical (http:app)"))
g4b checkpoint: firing webhook received and signature verified
g4b checkpoint: delivery audit: firing event be2d83fc-ee6b-4e5a-ac7d-384b1f564ff7 delivered after 1 attempt(s)
g4b checkpoint: g4-http.service restarted in the guest
g4b checkpoint: incident resolved
g4b checkpoint: resolved webhook received and signature verified
g4b checkpoint: vm stopped and deleted; g4b evidence complete
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 205.13s
```

Reading the observations:

- **The rule existed before the VM**: `f606b346…` was created before
  `vm.create` — the evaluator ran against a condition with no data and
  correctly produced nothing until the guest enrolled and reported
  (the missing-data policy in the rig's rule is `Unknown`, and the
  healthy baseline assertion proves no incident and no webhook
  existed before the outage).
- **The healthy baseline is real**: the app check reports ok with
  summary `HTTP 200` — the guest's loopback HTTP endpoint answering
  through the agent's local check engine, observed via the manager's
  check inventory.
- **The outage is a real OS transition**: `systemctl stop
  g4-http.service` over ssh; the check flips critical through the
  same collectors PR-5 qualified; the hold window (15 s) elapses; the
  evaluator opens incident `0c1fdc9b…` **firing** with severity
  `critical` and the VM as its resource, its `last_observed` carrying
  the check state (`critical (http:app)`).
- **The notification is signed and exactly once**: one firing webhook
  at the receiver, JSON body, `x-chv-signature` verified against an
  **independent HMAC-SHA256 the rig computes over the raw body** (not
  the dispatcher's signing path); the envelope names the incident,
  carries a non-empty event id, and its summary names the rule. The
  outbox row (`be2d83fc…`) is marked `delivered` after **1 attempt**
  — no retry storm, no duplicates, no dead letters.
- **The recovery is real**: `systemctl start` restores the endpoint,
  the check recovers, the 15 s recovery window elapses, and the
  incident **resolves** — exactly one signed resolved webhook, the
  same incident id as the firing envelope (identity carried end to
  end).
- **Teardown**: graceful `stop_vm` + `delete_vm`, bridge and tap
  removed, no leaked VMM process.

## Gate criteria vs. evidence (G4 part 2)

| G4 requirement | Evidence |
|---|---|
| Persistent incident behavior with deterministic alerts | The recorded run: rule created before boot; healthy baseline with no incident; the real outage holds then fires; the real recovery resolves — all through the durable store, observed via the manager's query paths; 85 evaluator tests pin the semantics (hold, recovery, missing ≠ zero, reset-safe rate) |
| No duplicate notification storms | Exactly one firing and one resolved webhook (asserted counts, not spot checks); dedup event ids and the outbox's delivered-once audit; the dispatcher suite pins duplicate/crash/backoff paths |
| Secure project-scoped actions | BFF route tests (operator/viewer tiers, revision 409, cross-tenant denial); the rig's notifications carry no credentials and the signature is verified independently |
| Signed webhooks | `x-chv-signature` verified against an independent HMAC-SHA256 over the raw body for both firing and resolved envelopes |

## Honest absences and findings (reported, not faked)

- **Optional external export** (prompt-05 task 7) is PR-8 — not faked
  here; `/metrics` is untouched by this PR.
- **vsock / multi-node transport** is prompt 06 scope (PR-7); this
  gate's guest path is outbound HTTPS over the bridge, as designed
  for v1.
- **Alerting retention** (resolved incidents, transitions, outbox
  rows) is unbounded in v1 — a runbook-documented boundary recorded
  as debt on #602.
- **Slack adapter**: exercised by the dispatcher's unit/integration
  suites (the same outbox/signing path); the real-VM rig drives the
  webhook channel, which is the signed-native one.

## Gate verdict

**G4 part 2 PASS** for the PR-6 scope: persistent incident behavior
with deterministic alerts on a real VM — a rule created before boot,
a healthy baseline with no incident and no webhook, a real service
outage holding then firing a critical incident, exactly one signed
firing webhook (independently verified HMAC), an honest delivery
audit, a real recovery resolving the incident with exactly one signed
resolved webhook carrying the same identity, and zero dead letters —
all through the production create → seed → boot → install → enroll →
ingest path on the qualified v53.0 pin, observed via the manager's
own query paths. No duplicate notification storms: the counts are
asserted, not observed. Optional external export (prompt-05 task 7)
is PR-8; vsock/multi-node is PR-7.
