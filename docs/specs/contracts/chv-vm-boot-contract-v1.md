# CHV VM boot contract v1

**Status:** Proposed  
**Authority:** ADR-022  
**Purpose:** define the normative VM boot-mode and boot-artifact contract

## 1. Scope

This contract defines how CHV represents and executes VM boot configuration.

It covers two modes:

- firmware boot;
- direct kernel boot.

This contract covers durable VM state, artifact identity, runtime resolution, compatibility, and qualification.

This contract does not define the root-volume format. Storage attachments remain authoritative for VM disks.

## 2. Normative terms

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

Use these product terms:

| Term | Meaning |
|---|---|
| VM boot mode | The semantic method CHV uses to start the operating system. |
| Firmware boot | Cloud Hypervisor loads firmware. The VM boot loader selects the kernel and initramfs. |
| Direct kernel boot | Cloud Hypervisor loads a Linux kernel directly. CHV also supplies the optional initramfs and command line. |
| Boot artifact | Immutable boot material identified by digest. |
| Boot artifact set | Immutable manifest that binds a direct-kernel kernel and optional initramfs to image compatibility metadata. |
| Resolved boot payload | Node-local paths produced from durable artifact identities immediately before VMM mutation. |

Do not use `ISO boot`, `external kernel boot`, `fast boot`, or `normal boot` as API or contract terms.

## 3. Boot-mode values

The durable and HTTP/JSON value is:

| Mode | JSON value | CLI value |
|---|---|---|
| Firmware boot | `firmware` | `firmware` |
| Direct kernel boot | `direct_kernel` | `direct-kernel` |

A new VM definition MUST contain one explicit mode.

CHV MUST NOT infer a new VM's mode from optional field presence after the v1 cutover.

## 4. Artifact reference

A v1 boot-artifact reference has this logical shape:

~~~json
{
  "digest": "sha256:<64 lowercase hex>",
  "size_bytes": 123456,
  "media_type": "application/vnd.chv.boot.kernel",
  "architecture": "x86_64"
}
~~~

The required fields are:

| Field | Rule |
|---|---|
| `digest` | MUST use `sha256:<64 lowercase hex>`. |
| `size_bytes` | MUST be greater than zero. |
| `media_type` | MUST identify the artifact kind. |
| `architecture` | MUST match the VM execution architecture. |

The v1 media types are:

- `application/vnd.chv.boot.firmware`
- `application/vnd.chv.boot.kernel`
- `application/vnd.chv.boot.initramfs`
- `application/vnd.chv.boot-set+json`

A digest identifies content. A display name MUST NOT be used as integrity identity.

## 5. Boot artifact set

Direct kernel boot references a boot artifact set.

The manifest has this logical shape:

~~~json
{
  "version": 1,
  "architecture": "x86_64",
  "kernel_release": "6.8.0-000-generic",
  "source_image_digest": "sha256:<image digest>",
  "kernel": {
    "digest": "sha256:<kernel digest>",
    "size_bytes": 12345678,
    "media_type": "application/vnd.chv.boot.kernel",
    "architecture": "x86_64"
  },
  "initramfs": {
    "digest": "sha256:<initramfs digest>",
    "size_bytes": 23456789,
    "media_type": "application/vnd.chv.boot.initramfs",
    "architecture": "x86_64"
  }
}
~~~

`initramfs` MAY be absent.

The set itself is content-addressed. `BootArtifactSetRef.digest` is the SHA-256 digest of the canonical manifest bytes.

CHV MUST resolve the kernel and initramfs from the same set.

CHV MUST NOT combine artifacts from different sets during VM start.

The manifest MUST record the source image digest when the set came from an imported image.

## 6. Firmware boot shape

The canonical firmware shape is:

~~~json
{
  "version": 1,
  "mode": "firmware",
  "firmware": {
    "digest": "sha256:<firmware digest>",
    "size_bytes": 1234567,
    "media_type": "application/vnd.chv.boot.firmware",
    "architecture": "x86_64"
  }
}
~~~

Rules:

- `firmware` is required.
- `boot_set` is forbidden.
- `cmdline` is forbidden at the CHV boot layer.
- CHV MUST map the resolved firmware path to Cloud Hypervisor `payload.firmware`.
- The VM disk owns its boot-loader, kernel, initramfs, and kernel command line.

## 7. Direct kernel boot shape

The canonical direct-kernel shape is:

~~~json
{
  "version": 1,
  "mode": "direct_kernel",
  "boot_set": {
    "digest": "sha256:<manifest digest>"
  },
  "cmdline": "console=ttyS0 console=hvc0 root=/dev/vda1 rw"
}
~~~

Rules:

- `boot_set` is required.
- `cmdline` is required and MUST NOT be empty after trimming.
- `firmware` is forbidden.
- The referenced set MUST contain a kernel.
- The referenced set MAY contain an initramfs.
- CHV MUST map the kernel to Cloud Hypervisor `payload.kernel`.
- CHV MUST map the initramfs to `payload.initramfs` when present.
- CHV MUST map `cmdline` to `payload.cmdline`.
- CHV MUST NOT add `payload.firmware`.

Direct kernel boot is Linux-only in v1.

## 8. Root volume separation

The VM boot contract never owns the root volume.

The root volume remains a normal storage attachment.

A direct-kernel boot definition can therefore use the same root volume as firmware boot.

Changing boot mode MUST NOT copy or rewrite the root volume as an implicit side effect.

Image preparation MAY read an image to extract boot artifacts. It MUST NOT patch the image to satisfy direct kernel boot.

## 9. Artifact materialization

The node resolves artifacts into a managed artifact root.

The exact root is deployment configuration. It is not part of the public API.

Materialization MUST follow these rules:

1. Validate the reference syntax.
2. Resolve or fetch into a temporary file inside the managed root.
3. Reject path traversal and symlink escape.
4. Verify size when known.
5. Compute SHA-256 over the completed file.
6. Compare the digest with the expected digest.
7. Make the completed object visible atomically.
8. Resolve the final managed path.
9. Build the Cloud Hypervisor payload only after every required artifact passes.

A digest mismatch MUST fail before VM mutation.

A missing artifact MUST fail before VM mutation unless the resolver can materialize the exact referenced digest.

## 10. Runtime-only resolved form

The hypervisor-facing layer MAY use this logical type:

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

This type is runtime-only.

The control plane MUST NOT persist it.

The Core journal MUST NOT write new host paths from this type.

The public API MUST NOT return these paths as the VM boot identity.

## 11. Image capability metadata

An imported image can report:

- `firmware` capability;
- `direct_kernel` capability;
- both;
- neither.

`direct_kernel` capability requires a verified boot artifact set.

Firmware capability means the image is bootable through the selected firmware profile. It does not require CHV to extract a kernel.

The image API SHOULD expose capability state without exposing node-local paths.

A VM-create request for `direct_kernel` MUST fail before scheduling or host mutation when the selected image lacks a compatible verified boot set.

## 12. Selection and fallback

Firmware boot is the default while direct kernel boot is unqualified.

A request for direct kernel boot is explicit.

CHV MUST NOT silently fall back between modes.

This rule applies to:

- missing artifacts;
- digest mismatch;
- unsupported architecture;
- invalid command line;
- node cache failure;
- VMM payload rejection.

The returned error SHOULD identify the failed precondition without exposing sensitive host paths.

## 13. Reboot and kernel-update semantics

Firmware boot delegates kernel selection to the VM boot loader.

Direct kernel boot pins the boot artifact set.

A reboot MUST use the same direct-kernel boot set unless an explicit CHV mutation changes it.

An in-VM package update MUST NOT silently modify CHV's pinned boot set.

The API MUST expose enough boot identity to let an operator determine which mode and boot set a VM uses.

Automatic reconciliation of mutable in-VM kernel package updates is outside v1.

## 14. Recovery and respawn

The authoritative VM definition includes:

- boot contract version;
- boot mode;
- firmware digest or boot-set digest;
- direct-kernel command line when applicable.

VMM respawn MUST reconstruct the same logical boot payload.

`chv-agent` restart MUST NOT choose a different local artifact because it is newer.

A missing required artifact after restart produces a recoverable precondition failure. It does not authorize fallback.

## 15. Snapshot and restore

A snapshot that claims VM recreation support MUST preserve the boot identity.

The preserved identity includes:

- boot contract version;
- boot mode;
- firmware digest or boot-set digest;
- direct-kernel command line.

Restore MUST resolve the same artifacts before VM execution.

A restore implementation that does not preserve this identity MUST state that limitation.

## 16. Migration

Before accepting VM execution, the destination node MUST resolve every required boot artifact.

The destination MUST verify the same digests as the source definition.

The destination MUST NOT substitute its configured default firmware or kernel.

Artifact prefetch MAY happen before migration cutover.

Artifact transfer is not the same as memory or disk migration. The implementation MAY use a separate content-addressed distribution path.

## 17. Legacy compatibility

The legacy shape is path-based:

~~~json
{
  "kernel": "/path/to/kernel",
  "firmware": null,
  "initial_disk": null
}
~~~

(`firmware` and `initial_disk` are optional and absent-or-null unless set; the interpretation table below carries the semantics.)

Legacy readers remain required during the migration window.

Interpretation is:

| Legacy state | Semantic mode |
|---|---|
| `firmware != null` | firmware |
| `firmware == null` | direct kernel |

This mapping is for compatibility only.

A legacy direct-kernel entry without initramfs and command-line data does not satisfy the v1 direct-kernel qualification contract.

New authoritative writes MUST stop using the legacy shape after the cutover gate.

Existing immutable journal records MUST NOT be edited in place.

A migration process MAY register legacy files as managed artifacts after it verifies their content digests.

## 18. Public API behavior

The normal create surface accepts `boot_mode`.

Example:

~~~json
{
  "image_id": "img-123",
  "boot_mode": "direct_kernel"
}
~~~

The control plane resolves image capability metadata into the durable boot identity before it accepts the VM definition.

Normal users select a mode. They do not supply host paths.

Low-level kernel, initramfs, or firmware path injection is not part of this contract.

## 19. CLI behavior

The CLI uses:

~~~text
chvctl vm create ... --boot-mode firmware
chvctl vm create ... --boot-mode direct-kernel
~~~

`chvctl vm show` SHOULD display:

- boot mode;
- boot artifact-set digest for direct kernel boot;
- firmware digest for firmware boot;
- kernel release when known.

The CLI MUST NOT print node-local managed artifact paths as normal VM identity.

## 20. Error classes

The implementation SHOULD distinguish these failures:

| Class | Example |
|---|---|
| invalid boot request | unknown mode or forbidden field combination |
| unsupported boot capability | image has no direct-kernel set |
| artifact unavailable | exact digest cannot be materialized |
| artifact integrity failure | resolved bytes do not match digest |
| architecture mismatch | artifact architecture differs from execution architecture |
| VMM boot payload rejected | Cloud Hypervisor rejects an otherwise resolved payload |
| legacy boot state incomplete | old direct-kernel record lacks required v1 data |

Errors MUST fail closed before VMM mutation when the failure is known before mutation.

## 21. Observability

A bounded label MAY use:

~~~text
boot_mode="firmware"
boot_mode="direct_kernel"
~~~

Do not use these as metric labels:

- VM ID;
- image ID;
- digest;
- host path;
- kernel release.

Logs MAY include a shortened digest for correlation. Security-sensitive paths SHOULD remain out of user-facing errors.

## 22. Qualification contract

A boot mode is not qualified by code presence alone.

Firmware boot retains its existing evidence status.

Direct kernel boot requires real-KVM evidence.

The minimum direct-kernel campaign must prove:

| Area | Required proof |
|---|---|
| image | unmodified supported cloud image |
| boot set | matching kernel and initramfs, with verified digests |
| create/start | VMM starts and reaches Linux kernel plus userspace marker |
| cloud-init | network and user-data apply |
| reboot | same boot-set digest is used |
| stop/start | VMM respawn uses the same boot identity |
| agent restart | recovery preserves mode and digests |
| integrity | corrupt artifact fails before mutation |
| availability | missing artifact fails without fallback |
| compatibility | firmware boot still passes its existing lifecycle path |
| timing | both modes measured on the same host and image family |

When migration or snapshot is claimed for direct kernel boot, the corresponding evidence must also prove artifact identity preservation.

The evidence records:

- CHV commit;
- Cloud Hypervisor version;
- host architecture;
- source image digest;
- boot mode;
- kernel release;
- firmware or boot-set digest;
- kernel digest;
- initramfs digest when present.

## 23. Support-state language

Use these support labels:

- `DESIGN-ONLY` before implementation;
- `CODE-SUPPORTED, UNQUALIFIED` after deterministic and integration tests pass;
- `QUALIFIED — KVM-VERIFIED` only after the real-KVM gate passes.

Do not describe direct kernel boot as production-ready because Cloud Hypervisor supports it upstream.

## 24. Non-goals

This contract does not define:

- Secure Boot for direct kernel boot;
- a general-purpose artifact registry;
- automatic in-VM kernel package tracking;
- a boot-time service-level objective;
- Windows direct kernel boot;
- arbitrary host-path injection;
- another VMM backend.

## 25. Related documents

- [ADR-022 — Dual VM boot model](../adr/022-dual-vm-boot-model.md)
- [Cloud Hypervisor reference](../ops/cloud-hypervisor-reference.md)
- [ADR-014 — API evolution and compatibility](../adr/014-api-evolution.md)
- [ADR-016 — Evolve `chv-agent` into CellHV Core](../adr/016-evolve-chv-agent-into-cellhv-core.md)
- [ADR-017 — Core compatibility invariants](../adr/017-core-compatibility-invariants.md)
