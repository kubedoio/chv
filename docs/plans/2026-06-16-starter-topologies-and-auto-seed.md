# Starter Topologies + Auto-Seed on First Deployment

**Date:** 2026-06-16
**Status:** Proposed (post-Phase-8 follow-up)
**Author:** Architecture Designer team
**Companion to:** [`2026-06-13-architecture-designer-implementation-plan.md`](2026-06-13-architecture-designer-implementation-plan.md)

---

## 1. Goal

When a CHV deployment boots for the first time, six **canonical reference
topologies** are pre-seeded into `architecture_topologies` so an operator
landing on `/architectures` sees a populated dashboard instead of an empty
state. Each starter is a fully-formed `CHVArchitecture` (YAML + design-graph
JSON) that can be inspected, cloned, validated, planned, and applied.

The starters double as **the canonical Designer demos**: each one walks the
operator through a distinct subset of CHV primitives (single VM, multi-tier,
multi-network, container orchestration, observability, edge) so the UI's
capability surface is discoverable on first contact.

## 2. The six starter topologies

The picks are grounded in independent reference-architecture research
(see [`/tmp/chv-starter-topology-research.md`](/tmp/chv-starter-topology-research.md)
or the appendix below). Each row is realizable with CHV primitives only —
no managed cloud services.

| # | Starter | Source(s) | Operator scope axis | VM count | Networks | Datastores |
|---|---------|-----------|---------------------|----------|----------|------------|
| 1 | **Single Linux Dev VM** | Azure "Run a Linux VM on Azure" | small dev | 1 | 1 bridge | 1 local |
| 2 | **LAMP / WordPress Single-Server** | AWS WordPress whitepaper; Hetzner & Vultr one-clicks | classic web | 1 | 1 bridge | 1 local + opt NFS |
| 3 | **Three-Tier Web (Web / App / DB)** | AWS WordPress whitepaper §"Public/App/Data subnets"; Azure N-tier Linux VM | n-tier | 3 | 2 (DMZ + internal) | local + NFS |
| 4 | **Kubernetes HA (Stacked etcd, 3+3)** | kubernetes.io/ha-topology; CNCF 2023 (84% K8s adoption) | container orchestration | 6 | 1–2 | 1 NFS shared PV |
| 5 | **Prometheus + Grafana Observability** | Hetzner Cloud Apps; CNCF top-growth project | data / observability | 2–3 | 1 | 1 NFS for TSDB |
| 6 | **K3s Single-Node Edge** | docs.k3s.io edge architecture | HA / edge | 1 | 1 VLAN | 1 local |

**Coverage rationale:** VM counts span 1 → 6+. Network counts span 1 → 2 with
both bridge and VLAN tagging exercised. Datastore types cover local, NFS, and
(implicitly) iSCSI as a swap-in for any NFS slot. **Every CHV primitive is
exercised by at least one starter** — the dashboard, by itself, becomes the
Designer's tutorial.

### What was deliberately rejected

| Rejected | Why |
|---|---|
| Serverless / FaaS reference architectures | CHV provides VMs, not managed services |
| OpenStack-style 1+1 starter cloud | docs.openstack.org disclaims as non-production; recursive (CHV-on-CHV) |
| Service mesh + serverless cloud-native | CNCF 2023: meshes declined 24%→21%, serverless 22%→13% — not what's deployed |
| MEAN as a separate starter | Structurally identical to LAMP — should be a workload variant of #2 |

---

## 3. Each topology, end-to-end

For each starter, this section gives:

- **YAML fixture** — exact `CHVArchitecture` content shipped under `crates/chv-architecture-validate/tests/fixtures/starters/`
- **Designer workflow** — the click-by-click sequence an operator can follow on the canvas to recreate it from scratch (also serves as Playwright-driver pseudocode)
- **Validation expectations** — which static checks fire, which fleet checks need a host present

### 3.1 Single Linux Dev VM (`starter-01-single-vm.yaml`)

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: starter-single-vm
  display_name: "Single Linux Dev VM"
  description: "One Linux VM on a flat bridge network with local storage. The minimum useful topology — your first VM."
  environment: development
  labels:
    starter: "1"
    archetype: "dev-box"
servers:
  - name: host-01
    role: compute
    resources: { cpu_cores: 4, memory_gb: 8 }
networks:
  - name: lan
    type: bridge
    bridge: br0
    cidr: 192.168.10.0/24
    gateway: 192.168.10.1
images:
  - name: ubuntu-24.04
    family: ubuntu
    version: "24.04"
templates:
  - name: small-linux
    image: ubuntu-24.04
    resources: { cpu_cores: 2, memory_gb: 4 }
    disk_gb: 20
instances:
  - name: dev-vm
    template: small-linux
    placement: { server: host-01 }
    networks:
      - { name: lan, ip: 192.168.10.10 }
```

**Designer workflow:**
1. New architecture → name `starter-single-vm`, environment `development`.
2. Drop a **Server** node from the palette → name `host-01`, role `compute`, 4 vCPU / 8 GiB.
3. Drop a **Network** node → `lan`, type `bridge`, bridge `br0`, CIDR `192.168.10.0/24`.
4. Drop an **Image** node → `ubuntu-24.04`.
5. Drop a **Template** node → reference `ubuntu-24.04`, 2 vCPU / 4 GiB / 20 GiB disk.
6. Drop an **Instance** node → template `small-linux`, place on `host-01`, attach NIC to `lan` with IP `192.168.10.10`.
7. Validate → expect zero errors. Save.

**Validation expectations:** all 14 static checks pass. Fleet check is `unknown` until a host is enrolled.

---

### 3.2 LAMP / WordPress Single-Server (`starter-02-lamp.yaml`)

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: starter-lamp-wordpress
  display_name: "LAMP / WordPress single-server"
  description: "Apache/MariaDB/PHP + WordPress on one VM. The most-deployed self-host workload (Vultr, Hetzner, AWS WordPress whitepaper)."
  environment: development
  labels: { starter: "2", archetype: "classic-web" }
servers:
  - name: host-01
    role: compute
    resources: { cpu_cores: 8, memory_gb: 16 }
networks:
  - name: web
    type: bridge
    bridge: br-web
    cidr: 10.20.0.0/24
    gateway: 10.20.0.1
datastores:
  - name: local-nvme
    type: local
    server: host-01
    capacity_gb: 200
images:
  - name: ubuntu-24.04
    family: ubuntu
    version: "24.04"
templates:
  - name: lamp-host
    image: ubuntu-24.04
    resources: { cpu_cores: 4, memory_gb: 8 }
    disk_gb: 60
    cloud_init:
      packages:
        - apache2
        - mariadb-server
        - php
        - php-mysql
        - libapache2-mod-php
        - wordpress
instances:
  - name: wp-01
    template: lamp-host
    placement: { server: host-01 }
    networks:
      - { name: web, ip: 10.20.0.10 }
```

**Designer workflow:**
1. Server `host-01` (8 vCPU / 16 GiB) → Network `web` (bridge `br-web`).
2. Datastore `local-nvme` attached to `host-01` (200 GiB local).
3. Template `lamp-host` with cloud-init `packages:` list — the Inspector pane
   exposes the `cloud_init.packages` field as a list editor.
4. Instance `wp-01` from template, NIC on `web` with `10.20.0.10`.
5. Validate, save.

**Why this exercises the Designer beyond #1:** introduces the **Datastore**
node and the **cloud-init Inspector pane** without the cognitive cost of
multiple VMs or networks.

---

### 3.3 Three-Tier Web (`starter-03-three-tier.yaml`)

The canonical multi-VM reference architecture — AWS WordPress whitepaper's
explicit Public / App / Data subnet split, restated as CHV primitives.

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: starter-three-tier-web
  display_name: "Three-tier Web (Web / App / DB)"
  description: "Public web tier, private app tier, isolated DB tier. Two networks (DMZ + internal) and a shared NFS datastore for the app tier."
  environment: staging
  labels: { starter: "3", archetype: "n-tier" }
servers:
  - name: host-web
    role: compute
    resources: { cpu_cores: 8, memory_gb: 16 }
  - name: host-app
    role: compute
    resources: { cpu_cores: 16, memory_gb: 32 }
  - name: host-db
    role: storage
    resources: { cpu_cores: 16, memory_gb: 64 }
networks:
  - name: dmz
    type: bridge
    bridge: br-dmz
    cidr: 10.30.0.0/24
    gateway: 10.30.0.1
  - name: internal
    type: vlan
    bridge: br-trunk
    vlan_id: 300
    cidr: 10.30.10.0/24
datastores:
  - name: app-shared
    type: nfs
    nfs_server: 10.30.10.250
    nfs_path: /export/app
    capacity_gb: 500
  - name: db-local
    type: local
    server: host-db
    capacity_gb: 1000
images:
  - name: ubuntu-24.04
templates:
  - name: web-tier
    image: ubuntu-24.04
    resources: { cpu_cores: 4, memory_gb: 8 }
    disk_gb: 40
  - name: app-tier
    image: ubuntu-24.04
    resources: { cpu_cores: 8, memory_gb: 16 }
    disk_gb: 80
  - name: db-tier
    image: ubuntu-24.04
    resources: { cpu_cores: 16, memory_gb: 64 }
    disk_gb: 200
instances:
  - name: web-01
    template: web-tier
    placement: { server: host-web }
    networks:
      - { name: dmz, ip: 10.30.0.10 }
      - { name: internal, ip: 10.30.10.10 }
  - name: app-01
    template: app-tier
    placement: { server: host-app }
    networks:
      - { name: internal, ip: 10.30.10.20 }
  - name: db-01
    template: db-tier
    placement: { server: host-db }
    networks:
      - { name: internal, ip: 10.30.10.30 }
```

**Designer workflow:**
1. Drop 3 Server nodes (web/app/db) — distinct sizing per row.
2. Drop 2 Network nodes — `dmz` (bridge) and `internal` (VLAN 300).
   *Inspector teaches the bridge-vs-VLAN distinction.*
3. Drop 2 Datastores — `app-shared` (NFS) and `db-local` (local on `host-db`).
4. Drop 3 Templates with role-appropriate sizing.
5. Drop 3 Instances. **`web-01` gets two NIC attachments** — one DMZ, one
   internal. *This is the first time the operator multi-NICs an instance —
   the canvas shows two edges from `web-01` to two different network nodes.*
6. Validate. Expect static check `INSTANCE_MULTI_HOMED` to fire as **info**
   (not error) — multi-homing is intentional here.

**Why this is the breakthrough demo:** first topology with **multiple
networks**, **multi-NIC instances**, and **mixed datastore types**. After
this one, the operator has seen 80 % of what the canvas can express.

---

### 3.4 Kubernetes HA (Stacked etcd, 3+3) (`starter-04-k8s-ha.yaml`)

Per `kubernetes.io/docs/setup/production-environment/tools/kubeadm/ha-topology/`,
stacked etcd is the simpler-to-operate canonical HA shape. CNCF 2023: 84 %
of organizations use or evaluate Kubernetes — this is the obligatory pick.

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: starter-k8s-ha
  display_name: "Kubernetes HA (3 control + 3 workers, stacked etcd)"
  description: "kubernetes.io stacked-etcd HA topology. 3 control-plane VMs run kube-apiserver/scheduler/controller-manager + colocated etcd. 3 worker VMs run kubelet + container runtime. NFS for persistent volumes."
  environment: staging
  labels: { starter: "4", archetype: "k8s-ha" }
servers:
  - name: hv-01
    role: compute
    resources: { cpu_cores: 24, memory_gb: 64 }
  - name: hv-02
    role: compute
    resources: { cpu_cores: 24, memory_gb: 64 }
  - name: hv-03
    role: compute
    resources: { cpu_cores: 24, memory_gb: 64 }
networks:
  - name: cluster-mgmt
    type: bridge
    bridge: br-cp
    cidr: 10.40.0.0/24
    gateway: 10.40.0.1
datastores:
  - name: pv-nfs
    type: nfs
    nfs_server: 10.40.0.250
    nfs_path: /export/pv
    capacity_gb: 2000
images:
  - name: ubuntu-24.04
templates:
  - name: k8s-control
    image: ubuntu-24.04
    resources: { cpu_cores: 4, memory_gb: 8 }
    disk_gb: 80
  - name: k8s-worker
    image: ubuntu-24.04
    resources: { cpu_cores: 8, memory_gb: 16 }
    disk_gb: 100
instances:
  - name: cp-01
    template: k8s-control
    placement: { server: hv-01 }
    networks: [{ name: cluster-mgmt, ip: 10.40.0.11 }]
    labels: { tier: "control-plane" }
  - name: cp-02
    template: k8s-control
    placement: { server: hv-02 }
    networks: [{ name: cluster-mgmt, ip: 10.40.0.12 }]
    labels: { tier: "control-plane" }
  - name: cp-03
    template: k8s-control
    placement: { server: hv-03 }
    networks: [{ name: cluster-mgmt, ip: 10.40.0.13 }]
    labels: { tier: "control-plane" }
  - name: worker-01
    template: k8s-worker
    placement: { server: hv-01 }
    networks: [{ name: cluster-mgmt, ip: 10.40.0.21 }]
    labels: { tier: "worker" }
  - name: worker-02
    template: k8s-worker
    placement: { server: hv-02 }
    networks: [{ name: cluster-mgmt, ip: 10.40.0.22 }]
    labels: { tier: "worker" }
  - name: worker-03
    template: k8s-worker
    placement: { server: hv-03 }
    networks: [{ name: cluster-mgmt, ip: 10.40.0.23 }]
    labels: { tier: "worker" }
```

**Designer workflow:**
1. Drop 3 Server nodes (host-01..03) — same sizing across all three.
2. Drop 1 Network node `cluster-mgmt`.
3. Drop 1 Datastore `pv-nfs` (NFS, 2 TiB, for ReadWriteMany PVs).
4. Drop 2 Templates: `k8s-control` and `k8s-worker`.
5. Drop **6 Instances**, three of each template, one per host — anti-affinity
   is encoded in the explicit `placement.server` distribution.
6. Validate. Expect static check `INSTANCE_LABEL_TIER_CONSISTENT` (info) —
   the canvas should colour-code by the `tier` label.

**Why this exercises the Designer beyond #3:** first **scale-out** topology
with **VM groups** (3 + 3) and **explicit anti-affinity placement**.
Demonstrates that the canvas scales past hand-counting — the dashboard's
finished-canvas screenshot of this one is the single best marketing asset.

---

### 3.5 Prometheus + Grafana Observability (`starter-05-observability.yaml`)

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: starter-observability
  display_name: "Prometheus + Grafana observability stack"
  description: "Two-VM observability stack: Prometheus on a TSDB-backed VM, Grafana fronting it. NFS-backed metrics retention. Pairs naturally with starter-04 (K8s)."
  environment: staging
  labels: { starter: "5", archetype: "observability" }
servers:
  - name: host-obs
    role: compute
    resources: { cpu_cores: 16, memory_gb: 32 }
networks:
  - name: monitoring
    type: bridge
    bridge: br-mon
    cidr: 10.50.0.0/24
    gateway: 10.50.0.1
datastores:
  - name: tsdb-nfs
    type: nfs
    nfs_server: 10.50.0.250
    nfs_path: /export/tsdb
    capacity_gb: 1000
images:
  - name: ubuntu-24.04
templates:
  - name: prometheus-host
    image: ubuntu-24.04
    resources: { cpu_cores: 8, memory_gb: 16 }
    disk_gb: 100
    cloud_init:
      packages: [prometheus, prometheus-node-exporter]
  - name: grafana-host
    image: ubuntu-24.04
    resources: { cpu_cores: 4, memory_gb: 8 }
    disk_gb: 40
    cloud_init:
      packages: [grafana]
instances:
  - name: prometheus-01
    template: prometheus-host
    placement: { server: host-obs }
    networks: [{ name: monitoring, ip: 10.50.0.10 }]
  - name: grafana-01
    template: grafana-host
    placement: { server: host-obs }
    networks: [{ name: monitoring, ip: 10.50.0.20 }]
```

**Designer workflow:** mirrors #3 in shape (1 server / 1 network / 1
datastore / 2 templates / 2 instances). The new content is the **distinct
cloud-init package list per template** and the **dedicated NFS datastore for
TSDB** — different attachment pattern than transactional DBs.

---

### 3.6 K3s Single-Node Edge (`starter-06-k3s-edge.yaml`)

```yaml
apiVersion: chv.kubedo.io/v1alpha1
kind: CHVArchitecture
metadata:
  name: starter-k3s-edge
  display_name: "K3s single-node edge"
  description: "Single-VM K3s with embedded SQLite — docs.k3s.io's canonical edge / branch-office topology. Demonstrates VLAN-tagged site segmentation."
  environment: development
  labels: { starter: "6", archetype: "edge" }
servers:
  - name: edge-host
    role: compute
    resources: { cpu_cores: 8, memory_gb: 16 }
networks:
  - name: site-vlan
    type: vlan
    bridge: br-trunk
    vlan_id: 600
    cidr: 10.60.0.0/24
    gateway: 10.60.0.1
images:
  - name: ubuntu-24.04
templates:
  - name: k3s-edge
    image: ubuntu-24.04
    resources: { cpu_cores: 4, memory_gb: 8 }
    disk_gb: 40
    cloud_init:
      runcmd:
        - "curl -sfL https://get.k3s.io | sh -"
instances:
  - name: edge-01
    template: k3s-edge
    placement: { server: edge-host }
    networks: [{ name: site-vlan, ip: 10.60.0.10 }]
```

**Designer workflow:** **VLAN configuration** is the new Inspector pane the
operator hits — bridge `br-trunk` + `vlan_id: 600`. The cloud-init
`runcmd` field (vs `packages` in #2 and #5) is the second new Inspector
control. Smallest topology that still teaches both VLAN and runcmd in one go.

---

## 4. Auto-seed mechanism

### 4.1 Where it runs

Inside `cmd/chv-controlplane/src/bootstrap.rs::build_service`, **after
`run_migrations` succeeds** and **before the service starts accepting RPC**:

```rust
// Existing:
run_migrations(&pool, Some(&store_config)).await?;

// NEW: seed starter topologies on first deployment.
let topology_repo = chv_controlplane_store::TopologyRepository::new(pool.clone());
chv_controlplane_seed::starters::seed_if_first_deployment(&topology_repo).await?;
```

The seeder lives in a **new crate `chv-controlplane-seed`** so the migration
crate stays storage-only and the bootstrap crate stays orchestration-only.
The crate ships with the 6 YAML fixtures embedded via `include_str!`.

### 4.2 First-deployment detection

Three options were evaluated:

| Option | How | Pros | Cons |
|---|---|---|---|
| **A. Sentinel row in `system_settings`** | New migration adds `(seed_starters_completed, "0")`; seeder updates to `"1"` | Explicit, durable, easy to override for tests | Needs a new migration |
| **B. Empty `architecture_topologies` table** | `if topology_repo.list(filter).count() == 0 { seed }` | Zero migration churn | An operator who archives every starter and re-boots gets re-seeded — surprising |
| **C. Filesystem marker `/var/lib/chv/.starters_seeded`** | Touch a file; check on boot | Survives DB rollback | Filesystem state outside the DB violates "DB is source of truth" |

**Decision: Option A.** The sentinel row is unambiguous, survives everything
the DB survives, and the new migration is one line. It also gives the
operator an explicit **opt-out** before first boot — set the row to `"1"`
manually before starting the service.

```sql
-- migrations/0052_seed_starter_topologies.sql
CREATE TABLE IF NOT EXISTS system_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT OR IGNORE INTO system_settings (key, value) VALUES ('seed_starters_completed', '0');
```

If the table already exists from another feature, the `CREATE TABLE IF NOT
EXISTS` is a no-op; the `INSERT OR IGNORE` keeps the migration idempotent
across re-runs and partial failures.

### 4.3 Seeder algorithm

```rust
pub async fn seed_if_first_deployment(repo: &TopologyRepository) -> Result<SeedOutcome, SeedError> {
    let setting: Option<String> = sqlx::query_scalar(
        "SELECT value FROM system_settings WHERE key = 'seed_starters_completed'"
    ).fetch_optional(repo.pool()).await?;

    if setting.as_deref() == Some("1") {
        tracing::info!("starter topologies already seeded; skipping");
        return Ok(SeedOutcome::Skipped);
    }

    let mut seeded = Vec::new();
    for fixture in STARTER_FIXTURES {
        match seed_one(repo, fixture).await {
            Ok(id) => seeded.push((fixture.name, id)),
            Err(SeedError::AlreadyExists { .. }) => {
                tracing::warn!(starter = %fixture.name, "starter already exists; skipping");
            }
            Err(e) => {
                // FAIL-OPEN: log and continue. A starter that won't seed must
                // not block the control plane from coming up.
                tracing::error!(starter = %fixture.name, error = %e, "starter seed failed");
            }
        }
    }

    sqlx::query("UPDATE system_settings SET value = '1', updated_at = $1 WHERE key = 'seed_starters_completed'")
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(repo.pool())
        .await?;

    tracing::info!(count = seeded.len(), "seeded starter topologies");
    Ok(SeedOutcome::Seeded { count: seeded.len() })
}
```

**Hard rules:**

1. **Fail-open per starter, fail-closed on the sentinel.** Individual seed
   failures log and continue; the sentinel is only flipped to `"1"` once the
   loop completes. A control plane that *can't read or update the sentinel*
   refuses to start (it's a DB-health signal).
2. **Owner is `NULL`** on every seeded row — starters belong to the system,
   not to the bootstrap admin user. The `architecture_topologies.owner_user_id`
   column is already nullable.
3. **Status is `draft`** on every starter. Apply requires an explicit
   operator action; the dashboard should not mass-apply on first boot.
4. **Environment defaults to `development`** for picks 1, 2, and 6;
   `staging` for picks 3, 4, and 5. **No starter ships as `production`** —
   prevents accidental admin-gated apply.
5. **Names are namespaced** with the `starter-` prefix and recorded as labels
   `starter: "<n>"`. Operators clone (not edit) starters to make their own.

### 4.4 Idempotency on re-boot

- `seed_starters_completed = '1'` → seeder is a no-op on every subsequent boot.
- An operator who deletes a starter and re-boots: **starter stays deleted.**
- An operator who wants the starters back: documented procedure
  `UPDATE system_settings SET value = '0' WHERE key = 'seed_starters_completed';
   service restart` in [`docs/OPERATIONS.md`](../OPERATIONS.md). The seeder's
   per-starter `AlreadyExists` branch means this only re-creates missing
   rows, not duplicates.

### 4.5 Why not a SQL-only seed (i.e. just put the YAML into the migration)?

Briefly considered. Rejected because:

- The YAML must round-trip through `parse_yaml` to produce
  `latest_yaml` + `design_graph_json`, and `parse_yaml` is Rust code.
  Hard-coding the `design_graph_json` into a SQL migration creates a
  long-lived divergence between what the migration writes and what
  `to_yaml(parse_yaml(yaml)) → yaml` produces today.
- Every fixture passes through Phase 7's `yaml_roundtrip` test on the way
  to seeding, so the seeded `design_graph_json` is guaranteed canonical.
- A Rust seeder can call the Phase-1 static-check pipeline before insert
  and **abort the seed of any starter that fails validation** — a SQL
  migration can't.

---

## 5. Designer workflow as a reusable pattern

The six per-topology workflows above all follow the same skeleton. The
Designer's tutorial system can encode it once and parameterize per starter:

1. **Choose archetype** — palette → "From starter…" → pick one of 6.
2. **Inspect** — read-only canvas + YAML view; no edits, no validation churn.
3. **Clone** — explicit "Clone as new architecture" button on the detail
   page. The clone gets the user's own `owner_user_id`, a fresh `id`, and
   `name = <starter>-clone-<short-uuid>` so the namespace stays clean.
4. **Edit** — the clone is an ordinary architecture: drag, rename, retune.
5. **Validate** — the Phase-1 static check pipeline runs.
6. **Plan** — Phase-4 plan tab → operator reviews diff.
7. **Apply** — Phase-5 apply tab → operator confirms.

Steps 2 and 3 are the new UI surface this plan implies:

- A **"Clone" button** on the detail page when `architecture.labels.starter`
  is set (read-only flag derived from labels).
- A **starter-aware empty state** on `/architectures` (only fires if the
  seed sentinel is `'0'` AND list returns empty — i.e. the seed was
  explicitly opted-out, not just deleted).

Both surfaces are small UI work — sized for a single PR.

---

## 6. Acceptance gates

```bash
# Crate-level (new chv-controlplane-seed)
rtk cargo test -p chv-controlplane-seed
rtk cargo test -p chv-controlplane-seed --test seed_idempotency
rtk cargo test -p chv-controlplane-seed --test fixtures_round_trip

# Workspace
rtk cargo build --workspace
rtk cargo test --workspace          # 798 → ~810 expected
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --all -- --check

# Integration — boot, expect 6 rows in architecture_topologies
rtk cargo test -p chv-controlplane-service --test starter_seed_integration

# UI
cd ui && rtk npm run check
cd ui && rtk npx vitest run
cd ui && rtk npx playwright test architectures-starters.spec.ts
```

The new Playwright spec asserts:

1. After a fresh boot, `/architectures` shows 6 cards with the right titles.
2. Each starter's detail page renders without errors and shows the canvas.
3. The "Clone" button on a starter creates a new architecture with status
   `draft`, distinct id, and the user as owner.
4. Deleting a starter and re-booting does not re-create it.

---

## 7. Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| YAML schema drift breaks an old starter on upgrade | Medium | Low (seeder logs + skips) | Phase 7 round-trip suite already covers all 6 fixtures; CI catches drift before release |
| Operator finds starter contents distracting on a real deployment | Medium | Low | `seed_starters_completed = '1'` set manually before first boot; documented in OPERATIONS |
| Seeder runs concurrently from two control-plane replicas | Low | Medium | The migration runs first under SQLite's exclusive lock; second replica sees sentinel = '1' and skips |
| Starter networks collide with the operator's real CIDR plan | Medium | Low | Each starter uses a distinct `10.X.0.0/24` (`10.10`, `10.20`, `10.30`, `10.40`, `10.50`, `10.60`) — easy to renumber per clone |
| Starter VM counts (esp. K8s HA at 6) overflow tiny test fleets | Medium | Low | Status is `draft` on seed; nothing is applied until the operator explicitly clicks Apply |

---

## 8. Phasing

This plan is sized for **one PR**, gated by reviewer pass:

| Stage | Files | Tests |
|---|---|---|
| **A. Seeder crate + fixtures** | `crates/chv-controlplane-seed/` (new), 6 YAML fixtures, migration `0052` | 1 fixture round-trip test, 1 seed-idempotency test, 1 boot-time integration test |
| **B. Bootstrap wire-up** | `cmd/chv-controlplane/src/bootstrap.rs` (1-line addition), Cargo.toml updates | Existing tests stay green |
| **C. UI surface** | "Clone" button + read-only starter banner on detail page; starter-aware empty-state copy | 1 Vitest unit test for the banner, 1 Playwright spec for the clone flow |
| **D. Docs** | `docs/OPERATIONS.md` opt-out procedure, README link to this plan | n/a |

**Reviewer pass:** language-specialist + test-analyzer (Rust seeder),
ui-design-engineer + reviewer-language-specialist (UI surface). No security
review needed — the seeder runs as the control plane's own user, and every
starter is `draft` status with no auto-apply.

---

## 9. Out of scope

- **Per-tenant starter sets** — every fresh CHV gets the same 6. Multi-tenant
  curation is a separate Phase post-MVP.
- **Starter localization** — descriptions ship in English. Internationalization
  follows the broader UI i18n project.
- **Marketplace / community starters** — the seeder ships exactly 6, embedded.
  A pluggable catalogue is a separate effort.
- **Auto-apply of any starter** — never. Status is always `draft` on seed.

---

## Appendix A — Source survey

(Verbatim from `/tmp/chv-starter-topology-research.md` — abridged.)

| Source | URL | Evidence |
|---|---|---|
| AWS WordPress whitepaper | docs.aws.amazon.com/whitepapers/latest/best-practices-wordpress/reference-architecture.html | Three-tier subnet split (Public/App/Data); CloudFormation reference templates |
| Azure Architecture Center | learn.microsoft.com/en-us/azure/architecture/reference-architectures/n-tier/linux-vm | "Run a Linux VM on Azure" — vNet + subnets + NIC + NSG + NAT + bastion |
| CNCF Annual Survey 2023 | cncf.io/reports/cncf-annual-survey-2023/ | 84% K8s adoption; meshes 24%→21%, serverless 22%→13% |
| kubernetes.io HA topology | kubernetes.io/docs/setup/production-environment/tools/kubeadm/ha-topology/ | Stacked etcd documented as "simpler to set up" |
| K3s architecture | docs.k3s.io/architecture | Single-node edge with embedded SQLite |
| Proxmox Cluster Manager | pve.proxmox.com/wiki/Cluster_Manager | "At least three nodes for reliable quorum" |
| Redis Sentinel | redis.io/docs/latest/operate/oss_and_stack/management/sentinel/ | "At least three Sentinel instances for a robust deployment" |
| OpenStack install-guide | docs.openstack.org/install-guide/overview.html | 1-controller minimum (non-production) |
| Hetzner Cloud Apps | docs.hetzner.com/cloud/apps | WordPress, LAMP, Docker, Coolify, Prometheus+Grafana, GitLab CE one-clicks |
| Vultr Marketplace | vultr.com/marketplace/ | LAMP, OpenLiteSpeed-WordPress, CloudPanel, Coolify |
| Stack Overflow Developer Survey | survey.stackoverflow.co/2024/technology and `/2025/technology` | Postgres 55.6%, MySQL 40.5%, Docker 71.1% (+17pp YoY) |

**Verified gaps** (flagged, not speculated): GCP Cloud Architecture Center
(JS-rendered), Helm Hub / Artifact Hub chart popularity (JS-rendered),
DigitalOcean and Linode marketplaces (JS-rendered). Vultr + Hetzner used as
proxies for self-host catalog signals.

---

## Appendix B — Decision log

| Decision | Rationale |
|---|---|
| Pick K8s stacked-etcd, not external | kubernetes.io explicitly labels stacked simpler; stays under 6 VMs |
| Pick K3s single-node, not K3s HA | The "edge" axis is the orthogonal coverage; K8s HA already represents large clusters |
| Status is `draft` on every starter | Apply is an operator decision, not a default |
| Owner is `NULL` on every starter | System-owned; clones get user ownership |
| Sentinel in `system_settings` | Survives DB rollback; explicit opt-out path |
| Rust seeder, not SQL migration | Validates each fixture through `parse_yaml` before insert |
| `starter-` name prefix + label | Easy filtering; clones can drop the prefix |
| 6 distinct CIDRs (`10.10..10.60`) | Easy to renumber; no overlap if operator deploys all 6 |
| Fail-open per starter, fail-closed on sentinel | A bad fixture must not block boot; a broken DB must |
