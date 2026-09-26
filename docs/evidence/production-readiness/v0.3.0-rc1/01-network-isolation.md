# CHV Production-Readiness — Prompt 01 Network Isolation & Host-Safety Evidence

> Campaign: [production-readiness](/docs/prompts/production-readiness/README.md)
> Prompt: [01-network-isolation](/docs/prompts/production-readiness/01-network-isolation.md)
> Capability maturity vocabulary: CODED / CI-VERIFIED / KVM-VERIFIED / MULTI-HOST-VERIFIED / RELEASED / FIELD-QUALIFIED.
> Evidence root for this candidate: `docs/evidence/production-readiness/v0.3.0-rc1/`
> Provisionally rated: **CI-VERIFIED + single-host KVM-VERIFIED (host-safety gate)**. MULTI-HOST-VERIFIED / FIELD-QUALIFIED remain **unproven on this infrastructure** (single physical host).

---

## 1. Root problem and baseline

### Problem (GitHub issue #227)

Before this change, `apply_firewall_rules` created the base `input`/`forward`/`output`
hook chains with `policy drop` and **no `iifname`/`oifname` ownership guard**. Applying any
CHV policy therefore dropped **all** host-stack, container/CNI, Docker, routing and
forwarded traffic passing the host netns — including SSH, kubelet, health checks and pod
traffic. It was not a confined tenant policy; it was a host-wide default-drop.

Captured as unsafe current-state in the Prompt 00 anchor table (`00-execution-declaration.md`
§1, #227 row) with the wording *"base chains input/forward/output `policy drop`, no
`iifname`/`oifname` guard → applying CHV policy drops all host/CNI/Docker/SSH traffic."*
The host also carries a **pre-existing unrelated** `hook forward policy drop` at nft
ruleset line 44 (Docker-style), which is why host-safety evidence below exercises the
unrelated **host-stack INPUT/OUTPUT** paths (see §5).

### Campaign baseline

| Item | Value | Source |
|---|---|---|
| Frozen evidence frame baseline `main` | `020e2b22a523b6e7e697a48bc2c020088bbf90e4` | Prompt 00 |
| Direct `main` parent before this merge | `0eb1eddc` (Prompt 00 evidence doc, PR #255) | `git log` |
| Implementation PR | **#256** — *fix(nwd): confine CHV firewall/NAT policy to CHV-owned guest traffic* | GitHub |
| Squash-merged `main` commit | `731a89cfe3fa602789be812e80822ed490f37dd2` (`731a89cf`) | `gh pr view` |
| Merged at | 2026-09-26T19:26:16Z | `gh pr view` |
| CI on merge commit | **green** (Build & Package, Rust checks, E2E tests, UI checks; KVM smoke not triggered on PR without the `kvm-test` label) | `gh pr checks 256` |

Pre-squash implementation history (all review iterations, for auditability):

```
472c6cc5 test(nwd): run concurrent fw+nat apply test on a multi-thread runtime
b93e702c test(nwd): concurrent fw+nat apply must persist both boundary halves
3dec3db4 fix(nwd): atomic policy_state updates and exact owned-iface resolution
a914df29 fix(nwd): fail-closed re-scope, fresh-table deny-first order, exposure hardening
ceda8c4d fix(nwd): replace (not duplicate) exposure records on re-expose
e04f0446 fix(nwd): fail-closed apply ordering, exposure re-assert, dnat family fix
f37707a3 fix(nwd): review hardening for CHV firewall/NAT confinement
17690191 fix(nwd): confine CHV firewall/NAT policy to CHV-owned guest traffic
```

---

## 2. Implemented design invariant

```
host nftables hooks: accept
        |
        +-- unrelated traffic ------------------> unaffected
        |
        '-- CHV-owned bridge/TAP/topology ------> CHV policy chains
                                                   |
                                                   '-- default deny
```

- **Ownership is authoritative.** The CHV-owned set for a topology is the bridge itself
  plus the members kernel-reported as enslaved via `ip link show master <bridge>`; the
  resolution **fails closed** (`NotFound`) when the bridge does not exist — CHV never
  guesses a host interface (Prompt 01 §2/§3).
- **No host-wide default-drop.** The three base hook chains are (re)created per apply with
  `policy accept`; after applying, a `verify_base_chain_policies` step **fails closed** if a
  stale `policy drop` base chain from an old daemon version survives an upgrade — it never
  applies new policy over a host-wide drop (the only remaining hazard from the old design
  is surfaced, not papered over).
- **`counter drop` terminals are installed before any dispatch or user rule.** On every
  apply each policy chain is flushed and immediately given its terminal `counter drop`
  before dispatch jumps / user rules are installed, so a mid-apply failure on a fresh table
  can never leave an empty drop-less policy chain (`fail-closed`, S-1).
- **Dispatch is guarded.** `iifname`/`oifname {owned}` jumps from `input`/`forward`/`output`
  into `chv-policy-in`/`chv-policy-fwd`/`chv-policy-out`. Both the host-stack paths and the
  **guest forwarding path** (the original miss in #227) are covered: inbound rules target
  `chv-policy-in` + `chv-policy-fwd`, outbound rules target `chv-policy-out` +
  `chv-policy-fwd`.
- **User-rule ordering is pinned.** `user_rule_insertion_plan` emits deny/reject rules
  first then accept (a broad accept can never shadow a specific deny), inserted head-first
  so the terminal drop is never displaced; `ct state established,related accept` is inserted
  last so it lands at the head (established guest flows survive policy replaces). Both
  invariants are unit-tested.
- **NAT is guarded too.** Masquerade matches only CHV-owned interfaces (the old
  "all non-loopback" host leak is gone).
- **Isolation by construction.** Each topology owns a distinct `chv-<network_id>` nft table;
  guards reference only that topology's resolved interfaces, so topology A cannot dispatch
  into topology B's chains. Deletion removes only the target topology's table/objects.

---

## 3. Required-work mapping (Prompt 01)

| Required work | Where implemented | Evidence |
|---|---|---|
| 1. Current-state proof | Prompt 00 anchor table documents the unsafe host-wide drop; `verify_base_chain_policies` unit tests guard the stale-`policy drop` upgrade case | `00-execution-declaration.md` §1; `firewall.rs` verify path + tests. A live regression test of the *old* unsafe effect is not preserved (the old code path is removed); the new confinement is proven live instead (§5). |
| 2. Scope policy to CHV-owned traffic | Base chains `policy accept`; guarded dispatch; forward path covered; per-topology tables; fail-closed ownership resolution | §4/§5; `host-safety-ruleset.txt` |
| 3. Input/privilege safety | Identifiers validated (`validate_rule`) and sanitized (`sanitize_id`/`sanitized_nft_table`); protocol allow-listed; target IP validated as `IpAddr`; all args passed via argv (no shell) — no expression / command injection, no host-global flush, no unverified-interface fallback | validation + parser unit tests |
| 4. Tests | 76 unit + 6 daemon integration tests; 3 privileged real-host tests | §4/§5 |
| 5. Observability | `NWD_NFT_ERRORS_TOTAL{operation=...}` for apply/apply-nat/re-assert/owned-resolution; `tracing` warns on flush failures; `HOST_SAFETY_DUMP_PATH` one-shot ruleset capture for host-safety evidence. No per-packet logging, no unbounded label sets. | code + `host-safety-ruleset.txt` |

### Prompt 01 §4 test-coverage checklist (privileged)

| Prompt-required coverage | Status | How proven |
|---|---|---|
| Empty CHV policy while SSH-equivalent host-stack traffic continues | ✅ | `confines_policy_to_chv_owned_traffic` (host INPUT ns-a→host 10.200.1.1) |
| Host egress/DNS-equivalent traffic continues | ✅ | same test (host OUTPUT host→ns-b 10.200.2.2) |
| Unrelated veth forwarding continues | ⚠️ qualified | Unrelated-forward-through-host is not isolatable on this box (pre-existing host `hook forward policy drop`); unrelated coexistence is proven on the host-stack INPUT/OUTPUT paths the old bug clobbered. Guest forwarding itself is proven via the CHV-owned forward dispatch + default-deny. |
| A second independent CHV topology is unaffected | ✅ by construction | Distinct per-topology nft tables + guards reference only own resolved interfaces; delete removes only own table. A live two-topology privileged run is not captured (see §9 residual). |
| CHV guest traffic defaults to deny with no allow rule | ✅ | `confines_policy_to_chv_owned_traffic`: guest `10.201.0.2`→gw ping denied under empty policy (`counter` drop terminals in dump) |
| Explicit allow rules work on the real path | ✅ | same test: `meta l4proto icmp ip saddr 10.201.0.0/24 accept` restores guest connectivity (visible in `host-safety-ruleset.txt` §2) |
| Repeated reconcile is idempotent | ✅ | same test re-apply; no duplicates, terminals re-installed, unrelated traffic still fine |
| Deleting one topology removes only its own objects | ✅ | cleanup deletes only its table/netns/links; `withdraw_service_exposure` marker-scoped delete; executor removes per-topology table on delete |
| Invalid identifiers fail before mutation | ✅ | `validate_rule` + `sanitize_id`/`sanitized_nft_table` unit tests |
| Missing topology/interface ownership fails closed | ✅ | `owned_ifaces_for_bridge("definitely-not-a-bridge-xyz")` → `NotFound`; unit test + real host |
| Kubernetes/CNI coexistence | ❌ not claimed | No K8s-capable disposable worker is available; per Prompt 01 and the campaign non-scope, coexistence is **not** claimed as qualified. |

---

## 4. Automated test evidence (merge commit `731a89cf`)

Run on this host, branch as merged:

```
cargo fmt --all                                          -> clean
cargo clippy -p chv-nwd-core --all-targets -- -D warnings -> clean
cargo test -p chv-nwd-core                                -> 76 unit + 6 daemon integration pass;
                                                             2 privileged host_safety + 1 privileged
                                                             owned_ifaces ignored (root-gated)
cargo check --workspace                                   -> ok
scripts/check-no-println.sh (ADR-009)                     -> passed (no println!/eprintln!)
```

The concurrent firewall+nat test (`concurrent_fw_nat_applies_persist_both_halves`) runs on a
2-worker multi-thread runtime and asserts that overlapping `set_firewall_policy` +
`set_nat_policy` both survive and are both re-asserted on NIC attach — stable across 10
local runs.

## 5. Real-host (privileged, sudo + nftables 1.0.9 + ip) evidence

This workspace host (`/dev/kvm`, 16 vCPU / 31 GiB) — the campaign's single KVM host.
All three privileged tests pass on the checked-out merge content:

```text
cargo build -p chv-nwd-core --tests
HS=$(ls -t target/debug/deps/host_safety-* | grep -v '\.d$' | head -1)
sudo -E env HOST_SAFETY_DUMP_PATH=.../host_safety_dump5.txt "$HS" --ignored --test-threads=1
  -> confines_policy_to_chv_owned_traffic ... ok
  -> exposure_survives_firewall_apply      ... ok
LIB=$(ls -t target/debug/deps/chv_nwd_core-* | grep -v '\.d$' | head -1)
sudo -E "$LIB" --ignored --exact executor::tests::owned_ifaces_resolves_bridge_and_enslaved_members
  -> ok
```

What is proven live on the host:

- **`confines_policy_to_chv_owned_traffic`** — two isolated netns (`us-a`, `us-b`) with
  host-end veths (`ha`, `hb`) plus a CHV bridge (`brhs`) with member (`gh`→`gs` guest ns).
  Empty policy ⇒ CHV guest traffic is default-dropped while unrelated host INPUT
  (ns-a→10.200.1.1) and host OUTPUT (host→10.200.2.2) **continue to pass**; the icmp allow
  rule restores the guest; a re-apply is idempotent and unrelated traffic is still untouched.
- **`exposure_survives_firewall_apply`** — a service exposure (forward-accept + prerouting
  DNAT) survives a later firewall apply that rebuilds the `forward` base chain (the
  re-assert path), and is cleanly withdrawn by marker.
- **`owned_ifaces_resolves_bridge_and_enslaved_members`** — the CHV-owned set resolves to
  **exactly** `{bridge, enslaved veth}` (sorted) and never includes the veth peer or any
  `link/ether`/`altname`/`inet` continuation-line token.

**Durable ruleset artifact:** `01-network-isolation/host-safety-ruleset.txt` — the real-host
`nft list table` capture showing for each apply: base chains `policy accept`, guarded
`jump`s from input/forward/output, `ct state established,related accept` at head, and the
terminal `counter packets 0 bytes 0 drop` on every policy chain; §2 shows the allow rule
(`meta l4proto icmp ip saddr 10.201.0.0/24 accept`) in-place between ct-established and the
drop terminal.

## 6. Acceptance-criteria reconciliation (Prompt 01)

| Criterion | Outcome |
|---|---|
| #227 acceptance criteria satisfied or superseded by stronger reviewed criteria | ✅ superseded by the stronger criteria in this doc and the reviewed confinement in `731a89cf` |
| No CHV policy path installs host-global default-drop | ✅ verified code + `verify_base_chain_policies` fail-closed + live dump shows `policy accept` bases |
| Unrelated host/namespace traffic remains functional in privileged tests | ✅ host INPUT/OUTPUT coexistence proven on the real host |
| CHV guest default-deny remains effective | ✅ proven live (empty-policy drops, allow restores) |
| Ownership and cleanup deterministic and idempotent | ✅ per-topology tables; idempotent re-apply; exact-owned-set resolution |
| The exact implementation commit has KVM/Linux-host evidence | ✅ `731a89cf` + real-host privileged runs + durable ruleset artifact |
| Documentation states the qualified coexistence profile and limitations | ✅ §5, §8, §9 |

## 7. Review history (all findings resolved)

Six comprehensive review rounds were run against the branch before merge; every round's
findings were fixed and verified. Highlights of the resolved classes (not exhaustive):

- **B1** — `ip link show master` field parsing (member names not ifindexes) + unit tests +
  real-host test.
- **S1** — unwritten-import guard definitions; **S2** — global `nft_lock` serializing all
  nft-mutating ops; **S4** — delete+recreate the base hook chains every apply (stale
  `policy drop` safety); **S5** — `policy_state` removal on topology delete; **S6** —
  cleanup guard before setup; IFNAMSIZ-safe names.
- Default-deny-first ordering (fresh-table fail-open); exposure re-assert after apply;
  family-correct `ip`/`ip6 daddr`; exposure record dedupe; forward-accept-before-DNAT.
- Desired-policy recorded on executor error (fail-closed re-scope on attach).
- **policy_state lost-update race** — atomic in-place `entry().or_default()` mutation +
  refresh re-reads latest before each apply.
- **owned-set continuation-line pollution** — parser accepts only `N:` header lines;
  real-host test asserts the exact set.
- Round 6 final gate: no blockers; the only SHOULD (test runtime) was fixed (multi-thread).

No open BLOCKER/SHOULD findings remain at merge time.

## 8. Failure / rollback behavior

- If ownership cannot be resolved (bridge missing) → the policy apply **fails closed** with
  `NotFound` before any nft mutation.
- If a mid-apply nft step fails → the error is propagated to the caller; the desired policy
  is already recorded so the next NIC attach re-scopes the boundary fail-closed; each policy
  chain retains its terminal drop.
- Deleting a topology removes only its own table/links/netns; exposure records are removed
  by marker.
- Rollback of the merge itself: `git revert 731a89cf` restores `main`; the change is a
  single squash commit.

## 9. Volume / residual risks & limitations (honest)

1. **Coexistence profile:** qualified on this single Linux host for CHV-owned VXLAN/local
   topologies and unrelated **host-stack** traffic. Kubernetes/CNI, Docker-forwarded, and
   multi-bridge coexistence are **not claimed** (no K8s-capable worker; Docker-style
   `hook forward policy drop` present on the host makes unrelated forward paths
   non-isolatable here).
2. **Live two-topology privileged test** is not captured on the real host; cross-topology
   isolation rests on per-topology tables + unit coverage of table/guard scoping.
3. **Transient CHV-guest unguarded window during re-apply:** recreating base chains before
   dispatch re-install is a documented, host-irrelevant transient (base policy stays
   `accept`; no host interface matches the guards). Converges on next successful apply;
   recorded in earlier reviews as a non-gating residual.
4. **Exposure binding** uses `iifname != lo` rather than a declared uplink (host port
   collision with host-bound traffic is a documented residual of the exposure feature, out
   of #227 scope); exposures are **memory-only** across daemon restarts (documented, tracked
   separately).
5. **KVM smoke** (`integration-kvm.yml`) is label-gated (`kvm-test`) and did not run on
   PR #256; the local privileged suite is the host-safety evidence. KVM-VERIFIED here means
   *host-safety gate* on this box, **not** a full VM-boot matrix (Prompt 04).
6. **MULTI-HOST-VERIFIED / FIELD-QUALIFIED** tiers are **unprovable on this single physical
   host** and are reported as unproven, per the Prompt 00 frame.

## 10. Evidence index

- `01-network-isolation.md` (this file).
- `01-network-isolation/host-safety-ruleset.txt` — real-host nft ruleset capture (merge content).
- `../00-execution-declaration.md` — frozen frame, anchor #227, baseline `main` SHA.
- Git: squash merge `731a89cf`; pre-squash history in §1.
- Review/CI records: `gh pr checks 256` (green), six review rounds (no open findings).
