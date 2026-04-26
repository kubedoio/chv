# Hypervisor Settings — Implementation Plan

> **Design spec**: `docs/superpowers/specs/2026-04-20-hypervisor-settings-design.md`  
> **Status**: Planning complete. Ready for subagent dispatch.

---

## 1. Architecture Summary

The feature introduces three database artefacts (migration already committed), extends the VM spec JSON pipeline, and adds a settings UI page. The data flow is:

```
UI (Settings page / VM modal)
  ↓  REST  /v1/settings/hypervisor  (BFF handlers)
BFF handlers  →  SQLite  hypervisor_settings / hypervisor_profiles
  ↓  (orchestrator builds AgentVmSpec)
AgentVmSpec JSON  →  gRPC CreateVmRequest.vm_spec_json
  ↓  (agent parses VmSpec)
VmSpec.hypervisor_overrides  →  VmConfig  →  CHV vm.create payload
```

Merge rule at orchestrator/agent boundary:  
`per-VM override (vms.hv_*)` → `global setting (hypervisor_settings)` → `hardcoded default`

---

## 2. Work Breakdown & Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 0  — Foundation (no deps)                                            │
│  ┌─────────┐  ┌─────────────┐  ┌─────────────┐                              │
│  │ 2.1 DB  │  │ 2.2 Spec    │  │ 2.3 Types   │                              │
│  │ Models  │  │ Structs     │  │ & Defaults  │                              │
│  └────┬────┘  └──────┬──────┘  └──────┬──────┘                              │
└───────┼──────────────┼────────────────┼─────────────────────────────────────┘
        │              │                │
        ▼              ▼                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 1  — Core Logic (depends on Layer 0)                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐                      │
│  │ 2.4 Merge   │  │ 2.5 Agent   │  │ 2.6 Validation  │                      │
│  │ Resolution  │  │ vm.create   │  │ (control plane) │                      │
│  │ (orch.)     │  │ Injection   │  │                 │                      │
│  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘                      │
└─────────┼────────────────┼──────────────────┼───────────────────────────────┘
          │                │                  │
          ▼                ▼                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 2  — API Surface (depends on Layer 1)                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │ 2.7 BFF Handlers│  │ 2.8 BFF Router  │  │ 2.9 create_vm   │             │
│  │ (get/update)    │  │ Registration    │  │ override support│             │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘             │
└───────────┼────────────────────┼────────────────────┼──────────────────────┘
            │                    │                    │
            ▼                    ▼                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 3  — Frontend (depends on Layer 2)                                   │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │ 3.0 API Client  │  │ 3.1 Settings    │  │ 3.2 VM Modal    │             │
│  │ & Types         │  │ Page            │  │ Advanced Tab    │             │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Detailed Work Items

### 2.1 Database Models — `crates/chv-controlplane-store/src/hypervisor_settings.rs` **NEW**

**Owner**: store layer  
**Deps**: migration `0021_hypervisor_settings.sql` (already committed)

Create a new store module with:

- `HypervisorSettingsRow` — `sqlx::FromRow` for the singleton `hypervisor_settings` table.
- `HypervisorProfileRow` — `sqlx::FromRow` for `hypervisor_profiles`.
- `HypervisorSettingsRepository` with methods:
  - `get_settings(&self) -> Result<HypervisorSettingsRow, StoreError>`
  - `update_settings(&self, input: &HypervisorSettingsPatchInput) -> Result<(), StoreError>`
  - `list_profiles(&self) -> Result<Vec<HypervisorProfileRow>, StoreError>`
  - `get_profile(&self, id: &str) -> Result<Option<HypervisorProfileRow>, StoreError>`
  - `apply_profile(&self, profile_id: &str) -> Result<(), StoreError>` — copies profile values into the singleton row
- `HypervisorSettingsPatchInput` — `Clone` struct with `Option<T>` for every column (only `Some` fields are updated).

Export from `lib.rs`:
```rust
pub use hypervisor_settings::{HypervisorSettingsRepository, HypervisorSettingsPatchInput, HypervisorProfileRow, HypervisorSettingsRow};
```

**Key design**: The `vms.hv_*` columns are read directly by the orchestrator JOIN — no repository method needed for per-VM overrides.

---

### 2.2 Spec Structs — `crates/chv-agent-core/src/spec.rs`

**Owner**: agent-core  
**Deps**: none

Extend `VmSpec` with an **optional** `hypervisor_overrides` field:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct HypervisorOverrides {
    pub cpu_nested: Option<bool>,
    pub cpu_amx: Option<bool>,
    pub cpu_kvm_hyperv: Option<bool>,
    pub memory_mergeable: Option<bool>,
    pub memory_hugepages: Option<bool>,
    pub memory_shared: Option<bool>,
    pub memory_prefault: Option<bool>,
    pub iommu: Option<bool>,
    pub rng_src: Option<String>,
    pub watchdog: Option<bool>,
    pub landlock_enable: Option<bool>,
    pub serial_mode: Option<String>,
    pub console_mode: Option<String>,
    pub pvpanic: Option<bool>,
    pub tpm_type: Option<String>,
    pub tpm_socket_path: Option<String>,
}
```

Add to `VmSpec`:
```rust
#[serde(default)]
pub hypervisor_overrides: Option<HypervisorOverrides>,
```

Add validation in `VmSpec::validate()`:
- `rng_src` if set must be non-empty and a valid path-like string.
- `tpm_type` if set must be one of `{"swtpm"}` (extensible enum).
- `serial_mode` if set must be one of `{"Pty", "File", "Off", "Null"}`.
- `console_mode` if set must be one of `{"Pty", "File", "Off", "Null"}`.

**Backward compatibility**: Old agents without this field in their code will still parse the JSON (unknown fields are ignored by `serde(default)`). The agent should be built with the new code before the control plane starts sending the field.

---

### 2.3 Types & Defaults — `crates/chv-common/src/hypervisor.rs` **NEW**

**Owner**: chv-common  
**Deps**: none

Create a shared types module so both control-plane and agent can reference the same defaults and validation constants:

```rust
pub const DEFAULT_CPU_NESTED: bool = true;
pub const DEFAULT_MEMORY_MERGEABLE: bool = false;
// ... etc for all fields

pub const VALID_SERIAL_MODES: &[&str] = &["Pty", "File", "Off", "Null"];
pub const VALID_CONSOLE_MODES: &[&str] = &["Pty", "File", "Off", "Null"];
pub const VALID_TPM_TYPES: &[&str] = &["swtpm"];
```

Add `pub mod hypervisor;` to `chv-common/src/lib.rs`.

---

### 2.4 Merge Resolution — `crates/chv-controlplane-service/src/orchestrator.rs`

**Owner**: controlplane-service  
**Deps**: 2.1, 2.2, 2.3

Modify `build_agent_vm_spec` to:

1. **Query per-VM overrides** by extending the `VmDesiredStateRow` query to JOIN `vms` and select all `hv_*` columns.
2. **Query global settings** from `hypervisor_settings` (singleton row).
3. **Merge** using the rule: `per-VM override` → `global setting` → `hardcoded default`.
4. **Populate** `AgentVmSpec.hypervisor_overrides` with the merged result (only non-default values need be sent to reduce payload size; sending all values is also fine).

The merge should be a deterministic function, e.g.:
```rust
fn resolve_hypervisor_setting<T: Clone>(vm_override: Option<T>, global: T) -> T {
    vm_override.unwrap_or(global)
}
```

For boolean fields, `None` on the VM means "use global".

---

### 2.5 Agent `vm.create` Injection — `crates/chv-agent-runtime-ch/src/process.rs`

**Owner**: agent-runtime-ch  
**Deps**: 2.2

Extend `VmConfig` (in `adapter.rs`) with `hypervisor_overrides: Option<HypervisorOverrides>`.

In `process.rs` `create_vm`, when building the `vm_config_json` payload for CHV `vm.create`, conditionally inject the hypervisor fields:

```rust
let mut vm_config_json = serde_json::json!({
    "cpus": {
        "boot_vcpus": config.cpus,
        "max_vcpus": config.cpus,
        // conditional:
        "topology": ... if hv.cpu_nested == Some(true) ...
    },
    "memory": {
        "size": config.memory_bytes,
        // conditional:
        "mergeable": hv.memory_mergeable,
        "hugepages": hv.memory_hugepages,
        "shared": hv.memory_shared,
        "prefault": hv.memory_prefault,
    },
    "payload": payload,
    "disks": disks_json,
    "net": net_json,
    "serial": { "mode": hv.serial_mode.as_deref().unwrap_or("Pty") },
    "console": { "mode": hv.console_mode.as_deref().unwrap_or("Off") },
    // conditional:
    "iommu": hv.iommu,
    "watchdog": hv.watchdog,
    "pvpanic": hv.pvpanic,
    "tpm": hv.tpm_type.as_ref().map(|t| json!({ "type": t })),
    "landlock": hv.landlock_enable,
});
```

**CHV v51.1 field mapping** (from `docs/superpowers/specs/2026-04-20-hypervisor-settings-design.md`):

| Setting | CHV REST Key | Notes |
|---------|-------------|-------|
| `cpu_nested` | `cpus.topology.threads_per_core = 1` + `kvm_hyperv` | CHV doesn't have a direct "nested" flag; nested virt is implied by topology + platform. Document as best-effort. |
| `cpu_amx` | `cpus.features.amx` | CHV v32+ |
| `cpu_kvm_hyperv` | `platform.kvm_hyperv` | boolean |
| `memory_mergeable` | `memory.mergeable` | boolean |
| `memory_hugepages` | `memory.hugepages` | boolean |
| `memory_shared` | `memory.shared` | boolean |
| `memory_prefault` | `memory.prefault` | boolean |
| `iommu` | `iommu` | boolean |
| `rng_src` | `rng.src` | path string |
| `watchdog` | `watchdog` | boolean |
| `landlock_enable` | `landlock` | boolean (CHV v37+) |
| `serial_mode` | `serial.mode` | "Pty"/"File"/"Off"/"Null" |
| `console_mode` | `console.mode` | "Pty"/"File"/"Off"/"Null" |
| `pvpanic` | `pvpanic` | boolean |
| `tpm_type` | `tpm.type` | "swtpm" |
| `tpm_socket_path` | `tpm.socket` | path string |

**Gate 2 (agent runtime)**: If CHV rejects a flag (e.g., version too old), the agent should surface the CHV error message in the operation failure. The agent does NOT silently drop fields — it sends what the control plane asked for and lets CHV validate.

---

### 2.6 Validation Module — `crates/chv-controlplane-service/src/hypervisor_settings_validator.rs` **NEW**

**Owner**: controlplane-service  
**Deps**: 2.3

Create a validation module for Gate 1 (API write-time):

```rust
pub fn validate_settings_patch(input: &HypervisorSettingsPatchInput) -> Result<(), ValidationError>;
pub fn validate_vm_overrides(overrides: &HypervisorOverrides) -> Result<(), ValidationError>;
```

Rules:
- **Type checking**: All booleans must be valid booleans (serde handles this, but explicit error messages help).
- **Mutual exclusion**: `tpm_type` and `tpm_socket_path` — if `tpm_type` is `None`, `tpm_socket_path` must also be `None`.
- **Enum validation**: `serial_mode`, `console_mode`, `tpm_type` must be in their respective valid sets.
- **Path validation**: `rng_src` if set must look like an absolute path (`/dev/...`).
- **Dependency**: `iommu=true` requires `memory_shared=true` for CHV to boot correctly (documented CHV requirement). Warn or error depending on strictness policy.

Export and wire into BFF handlers.

---

### 2.7 BFF Handlers — `crates/chv-webui-bff/src/handlers/hypervisor_settings.rs` **NEW**

**Owner**: webui-bff  
**Deps**: 2.1, 2.6

Create handler module with four endpoints:

#### `GET /v1/settings/hypervisor`
Returns the singleton row + list of profiles.

```json
{
  "settings": {
    "cpu_nested": true,
    "cpu_amx": false,
    ...
    "profile_id": "balanced"
  },
  "profiles": [
    { "id": "balanced", "name": "Balanced", "description": "...", ... },
    ...
  ]
}
```

#### `POST /v1/settings/hypervisor/update`
Accepts a partial patch (only fields to change). Runs Gate 1 validation. Updates `hypervisor_settings` row.

```json
{ "cpu_nested": false, "memory_hugepages": true }
```

#### `POST /v1/settings/hypervisor/apply-profile`
Accepts `{ "profile_id": "performance" }`. Copies profile values into the singleton row. Returns the updated settings.

#### `GET /v1/settings/hypervisor/profiles`
Returns all profiles (already included in the main GET, but useful for dropdowns).

All handlers require `require_operator_or_admin`.

---

### 2.8 BFF Router Registration — `crates/chv-webui-bff/src/router.rs`

**Owner**: webui-bff  
**Deps**: 2.7

Add routes:
```rust
.route("/v1/settings/hypervisor", get(crate::handlers::hypervisor_settings::get_settings))
.route("/v1/settings/hypervisor/update", post(crate::handlers::hypervisor_settings::update_settings))
.route("/v1/settings/hypervisor/apply-profile", post(crate::handlers::hypervisor_settings::apply_profile))
.route("/v1/settings/hypervisor/profiles", get(crate::handlers::hypervisor_settings::list_profiles))
```

---

### 2.9 `create_vm` Override Support — `crates/chv-webui-bff/src/handlers/vms.rs`

**Owner**: webui-bff  
**Deps**: 2.6

Modify `create_vm` handler to:

1. Accept optional `hypervisor_overrides` in the payload (same shape as `HypervisorOverrides` but JSON).
2. Run Gate 1 validation on the overrides via `validate_vm_overrides`.
3. Persist non-null override values into the `vms` table `hv_*` columns during the INSERT/UPDATE transaction.

Extend the transaction in `create_vm` to also update `vms` columns after the initial insert:
```sql
UPDATE vms SET
    hv_cpu_nested = ?,
    hv_cpu_amx = ?,
    ...
WHERE vm_id = ?
```

Only set columns where the payload provided an explicit value. Leave others as `NULL` (meaning "use global").

---

### 3.0 UI API Client — `ui/src/lib/bff/hypervisor-settings.ts` **NEW**

**Owner**: UI  
**Deps**: 2.8

Create BFF client module:

```typescript
export async function getHypervisorSettings(token?: string): Promise<HypervisorSettingsResponse>;
export async function updateHypervisorSettings(payload: HypervisorSettingsPatch, token?: string): Promise<HypervisorSettingsResponse>;
export async function applyHypervisorProfile(profileId: string, token?: string): Promise<HypervisorSettingsResponse>;
export async function listHypervisorProfiles(token?: string): Promise<HypervisorProfile[]>;
```

Add endpoint constants to `endpoints.ts`.

Add TypeScript types to `types.ts`:
```typescript
export type HypervisorSettings = {
  cpu_nested: boolean;
  cpu_amx: boolean;
  // ... all fields
  profile_id: string | null;
};

export type HypervisorProfile = {
  id: string;
  name: string;
  description: string;
  // ... all fields
  is_builtin: boolean;
};

export type HypervisorSettingsResponse = {
  settings: HypervisorSettings;
  profiles: HypervisorProfile[];
};

export type HypervisorSettingsPatch = Partial<HypervisorSettings>;
```

---

### 3.1 Settings Page — `ui/src/routes/settings/hypervisor/+page.svelte` **NEW**

**Owner**: UI  
**Deps**: 3.0

Create a new settings sub-page at `/settings/hypervisor`:

**Layout**:
- Left column: Global settings form with grouped sections:
  - **CPU**: `cpu_nested` (toggle), `cpu_amx` (toggle), `cpu_kvm_hyperv` (toggle)
  - **Memory**: `memory_mergeable`, `memory_hugepages`, `memory_shared`, `memory_prefault` (toggles)
  - **Devices**: `iommu` (toggle), `rng_src` (text input), `watchdog` (toggle), `pvpanic` (toggle)
  - **Security**: `landlock_enable` (toggle), `tpm_type` (select: none/swtpm), `tpm_socket_path` (text, conditional)
  - **Serial/Console**: `serial_mode` (select), `console_mode` (select)
- Right column:
  - **Profile selector**: Dropdown of profiles + "Apply" button. Shows profile description on hover.
  - **Current profile badge**: Shows which profile is active (if any).
  - **Reset to defaults** button.

**Interactions**:
- Each toggle/input saves on blur/debounce (PATCH semantics via `updateHypervisorSettings`).
- Applying a profile shows a confirmation modal: "This will overwrite your current global settings. Continue?"
- Reset to defaults calls `updateHypervisorSettings` with all fields set to their hardcoded defaults.

**Navigation**: Add a link in `/settings/+page.svelte` sidebar under "Quick Actions" or as a new card.

---

### 3.2 VM Modal Advanced Tab — `ui/src/lib/components/modals/CreateVMModal.svelte`

**Owner**: UI  
**Deps**: 3.0

Add a **Step 2b (Advanced)** or extend the existing step flow:

**Option A** (recommended): Add an "Advanced" accordion/section on Step 1 (Basic) that expands to show override fields. This keeps the 3-step flow intact.

**Option B**: Add a new Step 2b between Cloud-init and Review.

**Fields** (all optional, defaulting to "Use global setting"):
- Profile selector: "Use global" / "Balanced" / "Performance" / "Security Hardened" / "Custom"
- If "Custom" is selected, show the same toggle grid as the settings page.
- Each field has three states: unset (inherit global), true, false.

**Submission**: Include `hypervisor_overrides` in the `createVm` payload only for fields that were explicitly changed from "unset".

**Review step**: Show a summary line: "Hypervisor: Balanced profile" or "Hypervisor: Custom (4 overrides)".

---

## 4. Testing Strategy

### Unit Tests

| Module | Test |
|--------|------|
| `spec.rs` | Parse `VmSpec` with and without `hypervisor_overrides`; validate rejects bad enums. |
| `hypervisor_settings_validator.rs` | Test all validation rules: mutual exclusion, enum checks, iommu+shared dependency. |
| `orchestrator.rs` | Mock DB with global settings + per-VM overrides; assert merged spec JSON contains correct values. |
| `process.rs` (mock adapter) | Assert `vm.create` payload contains expected CHV keys when overrides are set. |

### Integration Tests

1. **Migration test**: Fresh DB applies `0021` and seeds defaults correctly.
2. **End-to-end API test**:
   - `POST /v1/settings/hypervisor/update` → changes global setting.
   - Create VM without overrides → agent spec contains global value.
   - Create VM with override → agent spec contains override value.
3. **Profile apply test**: Apply "performance" → global settings updated → new VMs inherit performance defaults.

### Manual UI Test

1. Open `/settings/hypervisor`, toggle `cpu_nested` off, refresh page → setting persists.
2. Apply "Security Hardened" profile → all toggles update, `tpm_type` shows "swtpm".
3. Create VM, open Advanced, set `memory_hugepages=true`, create → VM created successfully.
4. Check agent logs that `vm.create` payload includes `"memory": { "hugepages": true }`.

---

## 5. Rollout & Backward Compatibility

| Scenario | Behavior |
|----------|----------|
| Old agent, new control plane | Agent ignores unknown `hypervisor_overrides` JSON key. VM boots with CHV defaults (no regression). |
| New agent, old control plane | Agent receives `hypervisor_overrides: None`. Uses CHV defaults. |
| DB already has `0021` migration | Migration is idempotent (uses `CREATE TABLE IF NOT EXISTS`). |
| Downgrade control plane | `hypervisor_settings` table remains but is unused. Per-VM `hv_*` columns remain NULL. |

---

## 6. Open Questions / Decisions

1. **CHV `cpu_nested` mapping**: CHV doesn't expose a direct `nested` flag. We map it to `cpus.topology.threads_per_core=1` + `platform.kvm_hyperv=true` as best-effort. Document this in the UI tooltip.
2. **Profile copy vs reference**: Decision from design — profiles are **copied** into global settings on apply. Not a live reference. This simplifies the merge logic (no profile lookup at orchestrator time).
3. **IOMMU + shared memory dependency**: CHV requires `memory.shared=true` when `iommu=true`. Gate 1 validation should enforce this. If a user sets `iommu=true` via the UI, auto-enable `memory_shared` or show a blocking error?
   - **Recommendation**: Auto-enable with a toast warning: "IOMMU requires shared memory — enabled automatically."

---

## 7. File Checklist

### New files
- [ ] `crates/chv-controlplane-store/src/hypervisor_settings.rs`
- [ ] `crates/chv-common/src/hypervisor.rs`
- [ ] `crates/chv-controlplane-service/src/hypervisor_settings_validator.rs`
- [ ] `crates/chv-webui-bff/src/handlers/hypervisor_settings.rs`
- [ ] `ui/src/lib/bff/hypervisor-settings.ts`
- [ ] `ui/src/routes/settings/hypervisor/+page.svelte`
- [ ] `ui/src/routes/settings/hypervisor/+page.ts`

### Modified files
- [ ] `crates/chv-controlplane-store/src/lib.rs` — add exports
- [ ] `crates/chv-agent-core/src/spec.rs` — add `HypervisorOverrides`, update `VmSpec`
- [ ] `crates/chv-agent-runtime-ch/src/adapter.rs` — add `hypervisor_overrides` to `VmConfig`
- [ ] `crates/chv-agent-runtime-ch/src/process.rs` — inject CHV fields in `vm.create` payload
- [ ] `crates/chv-agent-runtime-ch/src/agent_server.rs` — map `VmSpec.hypervisor_overrides` → `VmConfig`
- [ ] `crates/chv-controlplane-service/src/orchestrator.rs` — merge logic in `build_agent_vm_spec`
- [ ] `crates/chv-webui-bff/src/router.rs` — add 4 new routes
- [ ] `crates/chv-webui-bff/src/handlers/vms.rs` — persist `hv_*` overrides in `create_vm`
- [ ] `ui/src/lib/bff/endpoints.ts` — add endpoint constants
- [ ] `ui/src/lib/bff/types.ts` — add TypeScript types
- [ ] `ui/src/lib/components/modals/CreateVMModal.svelte` — add advanced override section
- [ ] `ui/src/routes/settings/+page.svelte` — add hypervisor settings navigation link

---

*Plan written: 2026-04-20*  
*Ready for subagent dispatch.*
