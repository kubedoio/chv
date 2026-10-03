# ADR-022 — Dual VM boot model: firmware boot and direct kernel boot

## Status
Proposed

## Date
2026-10-03

## Context

CHV uses Cloud Hypervisor (the VMM) as its VM runtime. Cloud Hypervisor supports two Linux boot paths. It can start firmware and let the VM boot loader select the operating system. It can also load a Linux kernel directly.

CHV already exposes part of both paths, but the model is ambiguous. `cellhv-core-types::BootSpec` stores a required `kernel` string and an optional `firmware` string. The runtime treats `firmware = Some(...)` as firmware boot. It treats `firmware = None` as direct kernel boot.

The control plane currently always places `firmware_path` in the agent VM specification. Therefore the qualified production path is firmware boot. The direct-kernel branch exists in the runtime, but the durable model does not carry an initramfs or a kernel command line.

The real-host qualification also found a concrete firmware-chain limitation. The Noble qualification image is patched so GRUB uses `root=/dev/vda1` and omits its initramfs. The qualification script records direct kernel boot with initramfs support as a follow-up. This is test scaffolding, not a desirable image contract.

The current model also stores host paths inside VM definitions. A host path is not a stable VM identity. It is not safe for rescheduling, migration, restart recovery, or multi-node artifact distribution.

CHV needs two explicit boot modes. It also needs a stable artifact contract that keeps host-local paths outside durable VM state.

## Decision

CHV defines one VM boot model with two explicit modes:

| API value | Product name | Meaning |
|---|---|---|
| `firmware` | Firmware boot | Cloud Hypervisor loads firmware. The firmware starts the VM boot loader. The VM disk owns kernel selection. |
| `direct_kernel` | Direct kernel boot | Cloud Hypervisor loads the Linux kernel directly. CHV supplies the optional initramfs and the kernel command line. |

The CLI spelling is `firmware` and `direct-kernel`.

The generic term is **VM boot mode**.

Do not use these names as product terms:

- ISO boot
- external kernel boot
- fast boot
- normal boot
- kernel mode
- legacy boot

An ISO can be attached in either architecture. Boot speed is a consequence, not the semantic definition.

### 1. Boot semantics

Firmware boot uses this logical path:

~~~text
Cloud Hypervisor
  -> firmware
  -> VM boot loader
  -> kernel selected by the VM disk
  -> initramfs selected by the VM disk
  -> root volume
~~~

Direct kernel boot uses this logical path:

~~~text
Cloud Hypervisor
  -> CHV-selected kernel
  -> optional CHV-selected initramfs
  -> root volume
~~~

Direct kernel boot removes firmware and the VM boot loader from the operating-system boot path. It does not remove the root volume.

### 2. Durable boot model

CHV replaces the ambiguous path-based shape with an explicit tagged model.

The normative shape is equivalent to:

~~~rust
enum VmBootSpecV1 {
    Firmware {
        firmware: BootArtifactRef,
    },
    DirectKernel {
        boot_set: BootArtifactSetRef,
        cmdline: String,
    },
}
~~~

The serialized discriminator is `mode`. The allowed values are `firmware` and `direct_kernel`.

A new durable write MUST contain exactly one mode. A new durable write MUST NOT contain both firmware and direct-kernel fields.

The VM root volume stays in the storage attachment model. It MUST NOT move into `VmBootSpecV1`.

### 3. Boot artifacts

A **boot artifact** is immutable boot material identified by content digest. Examples are firmware, a Linux kernel, and an initramfs.

A **boot artifact set** is an immutable manifest for one direct-kernel boot combination. It binds these items:

- source image identity or source image digest;
- kernel artifact digest;
- kernel release;
- optional initramfs artifact digest;
- architecture;
- artifact-set manifest digest.

The kernel and initramfs MUST be treated as one versioned set. CHV MUST NOT combine unrelated kernel and initramfs artifacts at VM start.

The artifact digest format for v1 is `sha256:<64 lowercase hexadecimal characters>`.

A durable VM definition stores artifact identities. It MUST NOT store arbitrary node-local paths for new boot definitions.

### 4. Runtime resolution

The node runtime resolves durable artifact identities to host-local paths immediately before it creates the Cloud Hypervisor VM payload.

The resolved form is runtime-only. It is equivalent to:

~~~rust
enum ResolvedVmBootPayload {
    Firmware {
        firmware_path: PathBuf,
    },
    DirectKernel {
        kernel_path: PathBuf,
        initramfs_path: Option<PathBuf>,
        cmdline: String,
    },
}
~~~

`ResolvedVmBootPayload` MUST NOT cross a public API or enter durable desired state.

The resolver MUST verify the expected digest after materialization. It MUST fail before Cloud Hypervisor mutation when resolution or verification fails.

### 5. Cloud Hypervisor mapping

CHV maps firmware boot to the Cloud Hypervisor payload `firmware` field.

CHV maps direct kernel boot to the Cloud Hypervisor payload `kernel` field. It also sets `initramfs` when present and always supplies the accepted `cmdline`.

CHV MUST NOT implement firmware boot by putting firmware in the Cloud Hypervisor `kernel` field. Upstream permits some firmware binaries to work that way, but that does not make the semantic mode direct kernel boot.

CHV MUST NOT silently fall back from `direct_kernel` to `firmware`. The two modes have different trust, update, and performance semantics.

### 6. Image relationship

An image can advertise zero, one, or both boot capabilities.

Firmware support means the image contains a bootable disk layout for the selected firmware profile.

Direct-kernel support means CHV has a verified boot artifact set for that image. The artifact set must match the image architecture.

Image import MAY prepare a direct-kernel artifact set. Preparation includes extracting or registering the matching kernel and initramfs.

CHV MUST NOT mutate the imported root image only to make direct kernel boot work.

The current qualification image patch remains historical scaffolding. A qualified direct-kernel path must work without that GRUB patch.

### 7. Default selection

Firmware boot remains the default while ADR-022 is Proposed and while direct kernel boot is unqualified.

Direct kernel boot is explicit opt-in until its qualification gate passes.

A future ADR may change the default for image classes. ADR-022 does not authorize an automatic default change.

### 8. Update semantics

Firmware boot follows the VM disk's boot loader. A kernel update inside the VM can become active after reboot according to the VM's boot-loader policy.

Direct kernel boot pins the boot artifact set. A package update inside the root volume MUST NOT silently change the kernel that CHV injects.

A direct-kernel VM uses the same boot artifact set until an explicit CHV operation changes it.

CHV MUST reject a direct-kernel start when the pinned kernel release and the declared boot set are inconsistent with required compatibility metadata.

The v1 contract does not promise automatic extraction after every in-VM kernel package update.

### 9. Restart, recovery, snapshot, and migration

The boot mode and artifact identities are part of the authoritative VM definition.

Agent restart and VMM respawn MUST reconstruct the same mode and artifact identities.

Snapshot metadata that is intended to recreate a VM MUST preserve the boot mode and artifact identities.

A migration target MUST resolve and verify the same boot artifacts before it accepts VM execution. It MUST NOT substitute a locally preferred firmware, kernel, or initramfs.

A missing artifact is a precondition failure. It is not a reason to change boot mode.

### 10. Trust and integrity

Every managed boot artifact is immutable by digest.

Artifact materialization MUST use an atomic write pattern. The final path MUST not expose partially written data.

The resolver MUST reject symlink or path traversal that escapes the managed artifact root.

The resolver MUST verify the digest before it hands the path to Cloud Hypervisor.

Artifact signatures are a future policy layer. Digest verification is mandatory in v1.

The existing control-plane and `chv-agent` authority boundaries do not change. The compatibility layer does not gain direct Cloud Hypervisor access.

### 11. Compatibility with existing path-based state

Existing durable journal and VM-definition entries serialize the boot model as `boot.kernel`, `boot.firmware`, and `boot.initial_disk` (the `BootSpec` shape, matching contract §17). The agent cache and runtime spec instead use `kernel_path` and optional `firmware_path`. CHV must preserve recovery across the transition for both shapes.

Readers MUST continue to deserialize the legacy shape until the migration gate proves no required legacy state remains.

New writes MUST use the tagged v1 boot model after cutover.

Legacy entries with `firmware_path = Some(...)` map semantically to firmware boot.

Legacy entries with `firmware_path = None` map semantically to direct kernel boot, but they remain unqualified unless they also provide the required direct-kernel data.

The transition layer MAY resolve legacy configured files into managed artifacts. It MUST verify and register their digests before rewriting authoritative state.

CHV MUST NOT rewrite frozen journal history in place.

### 12. Public API and CLI

The public VM-create surface adds an explicit `boot_mode` with values `firmware` and `direct_kernel`.

The CLI uses:

~~~text
chvctl vm create ... --boot-mode firmware
chvctl vm create ... --boot-mode direct-kernel
~~~

Low-level host paths MUST NOT become normal CLI arguments.

A dedicated operator escape hatch for unmanaged development paths requires a separate contract. ADR-022 does not authorize one.

### 13. Observability

CHV records the selected boot mode as bounded-cardinality VM metadata or operation context.

Lifecycle evidence separates these timings:

- create acceptance to VMM-created;
- start request to first console output;
- boot start to Linux kernel banner;
- kernel banner to the configured userspace-ready marker.

Performance values are measurements. They are not correctness gates unless a separate acceptance document defines a threshold.

Metrics MUST NOT use artifact digests, VM IDs, image IDs, or paths as unbounded labels.

### 14. Qualification

Firmware boot keeps its current support level. ADR-022 does not upgrade or downgrade existing evidence.

Direct kernel boot starts as **CODE-SUPPORTED, UNQUALIFIED** only after the implementation exists. It becomes qualified only after real-KVM evidence passes.

The direct-kernel qualification must include:

1. An unmodified supported cloud image.
2. A matching kernel and initramfs artifact set.
3. Cloud-init networking and user-data.
4. Create, start, reboot, graceful stop, restart, and delete.
5. `chv-agent` restart and VMM re-adoption or respawn.
6. Failure on missing or digest-mismatched artifacts.
7. Failure without firmware fallback.
8. Recovery from persisted VM state.
9. Snapshot or migration checks when those features claim support for the tested profile.
10. Boot-stage latency measurements for both boot modes on the same qualification host.

The evidence must state the host, VMM version, image digest, kernel release, and artifact digests.

### 15. Architecture support

Direct kernel boot is Linux-only in v1.

Firmware boot remains the path for Windows and for operating systems that require their normal firmware or boot-loader chain.

Architecture support remains evidence-driven. The presence of an upstream Cloud Hypervisor feature does not create a CHV support claim.

## Consequences

### Benefits

- The API describes boot semantics instead of optional-path combinations.
- Direct kernel boot can reduce pre-kernel boot time.
- Firmware boot preserves normal VM boot-loader behavior.
- Content-addressed boot artifacts make recovery and migration deterministic.
- The model can remove the current qualification-only GRUB patch from the direct-kernel path.
- The runtime can prove which kernel and initramfs bytes it used.

### Costs

- CHV needs artifact preparation and node-local artifact caching.
- Direct-kernel VMs do not automatically follow kernel packages installed inside a mutable root volume.
- Existing durable path-based state needs a compatibility reader and a controlled cutover.
- Qualification must cover two boot paths instead of one.

## Guardrails

- Keep firmware boot working while direct kernel boot is added.
- Keep firmware as the default until direct kernel boot is separately qualified.
- Do not infer boot mode from path presence after the v1 cutover.
- Do not persist new arbitrary host paths in the VM boot model.
- Do not combine a kernel and initramfs from different boot artifact sets.
- Do not silently change a VM's boot mode.
- Do not silently fall back after a boot-artifact failure.
- Verify artifact digests before Cloud Hypervisor mutation.
- Preserve legacy journal readability during the transition.
- Keep the root volume in the storage model.
- Keep boot-mode metrics bounded in cardinality.

## Non-goals

- Replacing firmware boot.
- Changing the default boot mode in this ADR.
- Implementing Secure Boot for direct kernel boot.
- Automatically tracking kernel packages installed inside mutable VMs.
- Supporting arbitrary user-supplied host paths in the normal public API.
- Adding a second VMM.
- Replacing the CHV image service with a general artifact registry.
- Claiming a one-second service-ready boot target without measurement.

## Related documents

- [CHV VM boot contract v1](../contracts/chv-vm-boot-contract-v1.md)
- [Cloud Hypervisor reference](../ops/cloud-hypervisor-reference.md)
- [ADR-014 — API evolution and compatibility](014-api-evolution.md)
- [ADR-016 — Evolve `chv-agent` into CellHV Core](016-evolve-chv-agent-into-cellhv-core.md)
- [ADR-017 — Core compatibility invariants](017-core-compatibility-invariants.md)
- [ADR-020 — Per-VM systemd supervision and exclusivity](020-per-vm-systemd-supervision-and-exclusivity.md)
