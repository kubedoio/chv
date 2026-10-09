# NetBox qualification lane (ADR-024 decision 4, lane 3)

A disposable, pinned, real NetBox that the composed projection
scenarios run against — the "qualification" half of the #586
campaign's test strategy (`docs/specs/adr/024-netbox-test-doubles-and-demo-gate.md`):
the in-process simulator (lane 2) keeps every day fast and
deterministic, and this lane periodically proves the same scenarios
against the real thing, so simulator drift is caught by a tripwire
instead of by a production incident.

The lane is driven end-to-end by [`scripts/netbox-qualify.sh`]
(see `make netbox-qualify`); this directory only holds the compose
stack that stands the instance up.

## What runs

1. `docker compose up -d` boots the pinned stack:
   - NetBox `v4.7-5.1.1` (netbox-docker image; loopback port
     `127.0.0.1:18780` only),
   - PostgreSQL 18 and two Valkey instances (NetBox's queue + cache
     stores),
   - a one-shot `qualification-init` service that mints the fixed
     **v1** API token (40-char plaintext, `Authorization: Token`
     prefix — the auth scheme the `chv-netbox-adapter` client sends),
     provisions the `chv_` custom fields (text, on the six content
     types the projection writes — real NetBox 400s on writes that
     carry custom-field names that do not exist, so the adapter's
     ownership markers need the fields defined before any scenario
     can write), and publishes `NETBOX_QUALIFICATION_URL` +
     `NETBOX_QUALIFICATION_TOKEN` to the host via the shared
     `qualification-env` volume.
2. The script runs the five `qualification_*` wrappers in
   `chv-controlplane-service`'s composed suite (the same scenario
   code as the always-on simulator suite; `--ignored
   --test-threads=1`, serialized on a mutex, instance reset between
   scenarios).
3. With `--record`, the fixture recorder
   (`chv-netbox-sim`'s `record_netbox4_fixtures`) re-captures the
   golden fixtures under `crates/chv-netbox-sim/tests/fixtures/netbox4/`
   from the live instance, and the fixture fidelity tests must pass
   against the freshly recorded set before the stack is torn down.
4. On exit the script tears the whole project down
   (`docker compose down -v`) unless `--keep` was passed.

In CI, `.github/workflows/netbox-qualification.yml` runs the same
script on a weekly schedule (plus manual dispatch, with an optional
`record` input) and uploads the run log on failure.

## The fixed token

The API token is a fixed 40-character constant checked into the
compose file — deliberately. The instance is loopback-only,
disposable (destroyed with its volumes on every run), and seeded with
throwaway credentials; a per-run random token would add a
secret-distribution problem (the host script, the compose init
service, and the recorder all need the same value) without adding
any security to a stack whose DB password is equally public in the
same file. Real credentials never touch this lane.

## Manual use

```sh
# The whole lane (boots, tests, tears down):
make netbox-qualify

# Also re-record the golden fixtures from the live instance:
./scripts/netbox-qualify.sh --record

# Keep the instance up for poking at the UI (NetBox login:
# chv-qualification / chv-qualification-password):
./scripts/netbox-qualify.sh --keep
```

Prerequisites: a Rust toolchain (as pinned by `rust-toolchain.toml`),
`docker` with the compose plugin, and `curl`. The scenario suite's
`TestDb` is an in-memory SQLite store, so no database setup is
needed beyond what the compose stack itself runs.
