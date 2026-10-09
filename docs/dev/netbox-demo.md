# NetBox Demo (`make netbox-demo`)

An interactive harness that proves the NetBox projection integration is
**usable by a human** — configure → dry run → export → run history — with
zero NetBox installation. It is PR 4 of the #586 campaign
(plan: `docs/plans/2026-10-09-netbox-test-scenarios.md`; decision record:
[ADR-024](../specs/adr/024-netbox-test-doubles-and-demo-gate.md)).

```sh
make netbox-demo            # or: ./scripts/netbox-demo.sh
```

The script builds and boots, all on `127.0.0.1`:

| Piece | What it is |
|---|---|
| `chv-netbox-sim` | The stateful NetBox 4.x simulator (ADR-024 lane 2), on a fixed loopback port with a generated token |
| `chv-controlplane` | Built with `--features netbox-demo,dev` against a throwaway sqlite DB, serving the built Web UI (`ui/build`) |
| Seeded state | Bootstrap admin user, the first starter topology, a seeded *applied* version + succeeded apply run, and the NetBox projection config pointing at the simulator |

By default everything lives in a `mktemp -d` workspace that is removed
on exit. `--keep` preserves it (DB, credentials, logs), and
`--workspace <dir>` reuses a kept workspace: the simulator token is
re-read from its `demo.env` (so the stored projection config keeps
matching the sim), and the seed steps skip what already exists —
admin user, demo version, apply run, projection config. `Ctrl-C` tears
both processes down.

## What it proves

- The full UI → BFF → projection worker → NetBox loop works against a
  NetBox-shaped server that we did not script per request: the plan is
  computed from real (simulated) NetBox state, the run executes, and the
  resulting objects carry the `chv_*` ownership custom fields.
- The "can a human use it" gate from the #586 lane matrix — the same
  scenarios the composed test suite (`netbox_projection_sim_tests.rs`)
  proves mechanically, but driven by hand through the real UI.

## Prerequisites

- Rust toolchain (repo pin), `sqlite3`, `openssl`, `curl`, `jq`
- `python3` with **PyYAML** (YAML → normalized-model seeding) and
  **bcrypt** (`pip3 install bcrypt pyyaml`; `htpasswd` from
  `apache2-utils`/`httpd-tools` works as the bcrypt fallback)
- `node`/`npm` — only when `ui/build` is missing or stale; the script
  runs `cd ui && npm install && npm run build` in that case

## The click-path

1. Open the printed UI URL and log in with the printed demo credentials
   (also stored `0600` in the workspace as `admin_password`).
2. **Architectures** → open the seeded starter topology
   (`starter-01-single-vm` by default).
3. **NetBox** tab → **Configure**: the projection config is already
   seeded — endpoint = the simulator, token set, `mark_stale` retention.
   (The demo seeds the applied version + apply run the same way the
   composed test suites do, because a starter topology has no fleet to
   apply against.)
4. **Dry run** → the deterministic plan (creates / updates / no-ops /
   conflicts) computed live against the simulator.
5. **Export** → enqueues a projection run; **Run history** shows it
   reach `Succeeded` (the worker ticks every 30s).
6. Inspect what landed in "NetBox":

   ```sh
   curl -s http://127.0.0.1:18081/__state | jq '.objects.virtual_machines'
   ```

   Every projected object carries `chv_managed_by`, `chv_external_id`,
   `chv_architecture_id`, … — the ownership markers from
   ADR-023's mapping.

## Fault simulation (live)

The simulator's `__`-prefixed control plane is unauthenticated and
loopback-only. While the demo is up:

```sh
# Force 401 auth failures on every NetBox call, then press Dry run again:
curl -s -X POST http://127.0.0.1:18081/__faults \
     -H 'Content-Type: application/json' -d '{"auth_failure": true}' | jq .

# Simulate a NetBox outage for one kind only:
curl -s -X POST http://127.0.0.1:18081/__faults \
     -H 'Content-Type: application/json' \
     -d '{"kind": "virtual_machine", "server_error": 503}' | jq .

# Add latency to every response:
curl -s -X POST http://127.0.0.1:18081/__faults \
     -H 'Content-Type: application/json' -d '{"latency_ms": 2000}' | jq .

# Clear the GLOBAL fault config ('{}' replaces it with all-false).
# Per-kind entries are NOT touched by this — clear those separately:
curl -s -X POST http://127.0.0.1:18081/__faults \
     -H 'Content-Type: application/json' -d '{}' | jq .

# Clear a PER-KIND fault: an all-false entry for that kind replaces
# whatever the global config says for it:
curl -s -X POST http://127.0.0.1:18081/__faults \
     -H 'Content-Type: application/json' \
     -d '{"kind": "virtual_machine"}' | jq .

# Nuclear option — clear faults AND every object in the simulator:
curl -s -X POST http://127.0.0.1:18081/__reset | jq .
```

Watch how the controlplane behaves: a dry run against a *stopped*
simulator (or with `connection_drop` injected) answers
`502 NETBOX_UNREACHABLE` — a connect-level failure. A `server_error`
fault, by contrast, is an API-level response: the dry run answers a
5xx `INTERNAL_ERROR` instead. Either way, an exported run during an
outage is requeued with backoff and retried — never lost, never
duplicated.

## The plain-HTTP double gate (and why it never ships)

The production NetBox client is HTTPS-only and fail-closed: the token
must never travel over an unencrypted transport. The demo needs plain
HTTP because the simulator is a plain-HTTP loopback server. ADR-024
decision 5 gates that exception **twice**, because either gate alone is
a realistic accident:

1. **Compile-time** — the `netbox-demo` cargo feature (default-off) on
   `cmd/chv-controlplane`, forwarded through `chv-controlplane-service`
   and `chv-webui-bff` to the adapter's test-only plain-HTTP client
   constructor. Without the feature, the code path does not exist.
2. **Runtime** — even in a feature-enabled build, the demo client
   factories check `CHV_NETBOX_ALLOW_HTTP == "1"` on every use and fail
   closed with `NETBOX_HTTPS_REQUIRED` otherwise.

The script sets the env var only for the controlplane process it spawns,
and the startup log carries a loud marker:

```
NETBOX DEMO MODE: plain-HTTP NetBox client enabled (feature netbox-demo + CHV_NETBOX_ALLOW_HTTP=1) — NOT FOR PRODUCTION
```

This can never reach a shipped binary: **release packaging builds
default features only** (`docs/release/PIPELINE.md`), so the escape
hatch is absent from every packaged build. Any change to the gate
conditions (feature name, env var, factory seams) is a high-risk change
— see ADR-024's consequences and the disclosure requirements in
`CONTRIBUTING.md`.

## Options

```sh
./scripts/netbox-demo.sh --port 18080 --sim-port 18081 \
    [--no-seed] [--keep] [--workspace DIR]
```

- `--port` / `--sim-port` — controlplane HTTP / simulator ports
  (defaults `18080` / `18081`; the controlplane's gRPC port is `18443`).
- `--no-seed` — skip the demo architecture / projection-config seeding;
  you land on the UI with the six starters but no NetBox config. (The
  admin user is always seeded — the UI needs it to log in.)
- `--keep` — keep the workspace on exit; the workspace's `demo.env`
  records the URLs, the simulator token, and the seeded architecture id
  for shell-driven experiments.
- `--workspace DIR` — reuse a workspace (e.g. one preserved by
  `--keep`): its sqlite DB, admin credentials (recovered from
  `admin_password`), and simulator token (re-read from `demo.env`, so
  the stored projection config keeps matching the sim) are reused, and
  the seed steps skip what already exists. The dir is created if
  missing and is never deleted on exit (implies `--keep`). Default: a
  fresh `mktemp -d` workspace, removed on exit.

`CHV_NETBOX_DEMO_SKIP_BUILD=1` skips the cargo builds (binaries must
already exist in `target/debug`).
