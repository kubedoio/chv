# NetBox 4.x golden fixtures

These files pin the wire shapes the simulator produces on the six
endpoint families of the mapping contract's "NetBox REST surface used
(v1)" section (`docs/specs/architecture-designer/contracts/netbox-mapping-contract.md`,
"Simulator conformance").

## Contents

| File | Pins |
|---|---|
| `dcim-devices.json` | `GET /api/dcim/devices/` list envelope + Device object shape |
| `virtualization-virtual-machines.json` | `GET /api/virtualization/virtual-machines/` + VirtualMachine shape |
| `virtualization-interfaces.json` | `GET /api/virtualization/interfaces/` + Interface shape |
| `ipam-prefixes.json` | `GET /api/ipam/prefixes/` + Prefix shape |
| `ipam-vlans.json` | `GET /api/ipam/vlans/` + VLAN shape |
| `ipam-ip-addresses.json` | `GET /api/ipam/ip-addresses/` + IPAddress shape (incl. derived `assigned_object`) |
| `seed.json` | The **input** that produces the six responses above (seed-file format, not a captured response) |

## Provenance

**Not captured from a live NetBox.** These fixtures were authored for
PR 2 of the #586 campaign (ADR-024) by deriving the shapes from:

1. the mapping contract's documented examples and object table
   (natural keys, custom fields, ownership markers), and
2. the adapter client's fail-closed parser requirements
   (`crates/chv-netbox-adapter/src/client.rs`): `id`, kind-specific
   required fields (`name`, `status.value`, `virtual_machine.name`,
   `prefix`, `vid`, `address`), nested relation `.name`/`.vid` reads,
   scalar-only `custom_fields`, and the `{count, next, previous,
   results}` envelope, plus the wire examples in
   `chv-netbox-adapter`'s `client.rs` in-crate tests and the
   `remote_fixture` helper in `chv-controlplane-service`'s e2e suite.

They are, by ADR-024's own terms, **hypotheses**: the qualification
lane (PR 5) runs the same scenarios against a pinned real NetBox and
re-captures this directory via `scripts/netbox-qualify.sh --record`.

## Rules

- Never hand-edit these files after a real capture. Refresh them only
  through the qualification `--record` mode against a live instance.
- When the mapping contract changes, the simulator and these fixtures
  change in the same PR.
- `tests/fixture_tests.rs` asserts round-trip fidelity: the running
  simulator's list responses (seeded from `seed.json`) serialize to
  exactly these shapes. The only normalization is the ephemeral
  `http://127.0.0.1:<port>` base (from the test's ephemeral port)
  replaced with the stable `http://netbox.example.com` base used in
  the fixtures — a recorded capture would show the live instance's
  own host there instead.

## Known fidelity notes (to be confirmed or corrected by `--record`)

- Nested relations (`site`, `cluster`, `device`, `virtual_machine`,
  VLAN-on-prefix, `assigned_object`) are emitted in a minimal
  `{id, name, …}` form containing exactly the fields the contract's
  parser reads; a live NetBox emits fuller nested serializers.
- Tags are emitted as nested `{id, name, slug}` objects (NetBox 4.x
  list-endpoint form).
- The default page size (50) and server max page (1000) mirror
  NetBox's `PAGINATE_COUNT` / `MAX_PAGE_SIZE` defaults.
