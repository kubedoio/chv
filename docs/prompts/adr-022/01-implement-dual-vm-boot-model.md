# Prompt 01 — Implement ADR-022 dual VM boot model

Implement ADR-022 and the CHV VM boot contract v1 without weakening the existing firmware-boot path.

## Preconditions

Read these files before changing code:

- `AGENTS.md`
- `docs/specs/adr/022-dual-vm-boot-model.md`
- `docs/specs/contracts/chv-vm-boot-contract-v1.md`
- `docs/specs/ops/cloud-hypervisor-reference.md`
- `docs/governance/DOCUMENTATION_STANDARD.md`

ADR-022 and its contract are the design authority.

Do not start implementation from this prompt alone. If ADR-022 is not present on your base branch, rebase onto the branch or commit that contains it.

Use the current qualified Cloud Hypervisor version. Do not upgrade the VMM as part of this work.

Do not edit frozen evidence, released history, old plans, analysis documents, or previously executed prompts.

## Goal

Add two explicit VM boot modes:

- `firmware`
- `direct_kernel`

Keep firmware boot behavior working.

Make direct kernel boot a real CHV feature with kernel, optional initramfs, and kernel command line support.

Use content-addressed boot artifacts for new durable state.

Do not expose arbitrary host paths in the normal public API.

Do not silently fall back between boot modes.

## Current facts that must be re-verified

The implementation starts from these observed facts. Verify them against the actual base commit before coding.

1. `cellhv-core-types::BootSpec` currently stores `kernel`, optional `firmware`, and optional `initial_disk`.
2. `chv-agent-core::VmSpec` currently stores `kernel_path` and optional `firmware_path`.
3. The control-plane orchestrator currently always sends `firmware_path = Some(...)` for normal VM dispatch.
4. `ProcessCloudHypervisorAdapter` maps firmware to Cloud Hypervisor `payload.firmware`.
5. The same adapter maps the no-firmware case to `payload.kernel`.
6. The adapter does not yet add `payload.initramfs` or `payload.cmdline` for direct kernel boot.
7. The current real-host qualification path uses firmware boot.
8. The qualification image patch removes initramfs use. Do not treat that patch as the target direct-kernel design.
9. The image import path is currently lightweight metadata management. Do not invent a mature distributed image service that does not exist.

If any fact changed, document the difference in the implementation PR before adapting the plan.

## Non-negotiable invariants

- `chv-agent` remains the VM lifecycle authority.
- Cloud Hypervisor remains the only active VMM backend.
- The root volume remains a storage attachment.
- Firmware boot remains the default until direct kernel boot is KVM-qualified.
- New direct-kernel state uses an explicit tagged mode.
- New durable boot identity uses content digests, not arbitrary host paths.
- A digest mismatch fails before VMM mutation.
- Missing direct-kernel artifacts fail without firmware fallback.
- Kernel and initramfs come from one boot artifact set.
- Agent restart and VMM respawn preserve the same boot identity.
- Old journal and cache state remains readable during migration.
- Existing immutable journal history is never rewritten in place.
- Metrics do not use VM IDs, digests, paths, image IDs, or kernel releases as unbounded labels.
- No direct-kernel support claim becomes `QUALIFIED — KVM-VERIFIED` without real-KVM evidence.

## Required implementation sequence

Implement this work as narrow reviewable PRs. Do not submit one monolithic feature PR.

### PR A — Domain model and compatibility reader

Introduce the v1 boot types in the domain layer.

Use an explicit semantic model equivalent to:

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

Add typed artifact identities.

At minimum, represent:

~~~rust
struct BootArtifactRef {
    digest: String,
    size_bytes: u64,
    media_type: BootArtifactMediaType,
    architecture: String,
}

struct BootArtifactSetManifest {
    version: u32,
    architecture: String,
    kernel_release: String,
    source_image_digest: String,
    kernel: BootArtifactRef,
    initramfs: Option<BootArtifactRef>,
}
~~~

Use the contract's media types and digest syntax.

Validation must reject:

- unknown modes;
- empty direct-kernel command line;
- invalid digest syntax;
- zero artifact size;
- firmware fields in direct-kernel mode;
- direct-kernel fields in firmware mode;
- architecture mismatches inside one boot set;
- a missing kernel in a direct-kernel set.

Preserve legacy reads.

The old path-based shape must continue to deserialize for recovery.

Do not let compatibility force new writes back into the old shape.

Prefer an explicit compatibility adapter or custom deserializer over optional-field inference throughout the new code.

Add deterministic tests for:

- v1 firmware round trip;
- v1 direct-kernel round trip;
- legacy firmware read;
- legacy no-firmware read;
- malformed mixed modes;
- invalid digests;
- old journal payload compatibility.

Do not add filesystem I/O to the pure domain crate.

### PR B — Managed boot artifact store and resolver

Add a node-local content-addressed boot artifact store.

Use a configurable managed root. A recommended default is under `/var/lib/chv/`.

Do not make the public API depend on that path.

Implement a narrow resolver interface.

A suitable shape is:

~~~rust
#[async_trait]
trait BootArtifactResolver {
    async fn resolve_artifact(
        &self,
        artifact: &BootArtifactRef,
    ) -> Result<PathBuf, ChvError>;

    async fn resolve_boot_set(
        &self,
        set: &BootArtifactSetRef,
    ) -> Result<ResolvedBootSet, ChvError>;
}
~~~

Materialization must:

1. stay inside the managed root;
2. reject symlink escape and path traversal;
3. write through a temporary file in the managed root;
4. verify size when declared;
5. hash the completed bytes;
6. compare the exact digest;
7. publish atomically;
8. never expose a partially written final object.

Do not log full artifact contents.

Do not place artifact digests into Prometheus labels.

Add tests for:

- cache hit;
- first materialization;
- concurrent same-digest materialization;
- corrupt artifact;
- size mismatch;
- symlink escape;
- partial-write cleanup;
- restart with an existing valid artifact;
- restart with an invalid cached artifact.

Do not claim multi-node artifact distribution in this PR.

If the repository has no trustworthy distribution path, implement local managed resolution only and fail closed on a node without the artifact.

Record that support boundary clearly.

### PR C — Image boot metadata and preparation

Extend image metadata so an image can advertise:

- firmware capability;
- direct-kernel capability;
- both;
- neither.

Do not infer direct-kernel capability from an image name.

A direct-kernel capability requires a verified boot artifact set.

Add an image preparation path that can register a matching kernel and optional initramfs for a supported Linux image.

The first implementation may use a bounded extraction helper.

It must:

- inspect the image read-only;
- identify the selected kernel;
- identify the matching initramfs when required;
- record kernel release;
- compute digests;
- build the canonical boot-set manifest;
- register the manifest by digest;
- leave the root image unchanged.

Do not patch GRUB.

Do not delete initramfs lines.

Do not modify the source root image.

Keep image preparation separate from VM start.

If safe generic extraction cannot be implemented for all images, use an explicit supported-image profile and reject everything else.

Do not silently guess.

Add tests for:

- supported image preparation;
- image with no usable kernel;
- mismatched kernel and initramfs;
- unsupported architecture;
- stable manifest digest;
- repeated preparation as a no-op;
- source image remaining byte-identical.

### PR D — Control-plane desired state, API, and CLI

Add `boot_mode` to VM create.

HTTP/JSON values:

- `firmware`
- `direct_kernel`

CLI values:

- `firmware`
- `direct-kernel`

Firmware remains the default.

Do not accept normal-user host paths for kernel, initramfs, or firmware.

Resolve image boot capability before accepting new v1 desired state.

For direct kernel boot:

- require a compatible boot set;
- persist the boot-set digest;
- persist the command line;
- do not persist node-local paths.

For firmware boot:

- persist a firmware artifact identity for new v1 state;
- do not persist the configured host path as new boot identity.

Use an additive database migration.

Do not force existing rows into fake v1 state during SQL migration.

A safe transition is:

- legacy rows keep v1 boot columns null;
- new rows set `boot_contract_version = 1`;
- new rows set explicit mode and digest identity;
- dispatch recognizes legacy rows through the compatibility path;
- a later migration can remove legacy support after evidence proves it safe.

Do not make new NOT NULL constraints depend on values that a SQL migration cannot derive from host files.

Update `chvctl vm show` or the equivalent VM detail surface to show:

- boot mode;
- firmware digest or boot-set digest;
- kernel release when known.

Do not show managed local artifact paths as VM identity.

Add API and CLI tests for:

- omitted mode defaults to firmware;
- explicit firmware;
- explicit direct kernel;
- unknown mode;
- direct kernel on image without boot set;
- no path injection;
- old VM row still dispatches through the legacy path.

### PR E — Runtime resolution and Cloud Hypervisor payload

Add an explicit runtime type equivalent to:

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

Resolve artifacts before creating the Cloud Hypervisor VM payload.

Map:

~~~text
firmware
  -> payload.firmware

direct_kernel
  -> payload.kernel
  -> payload.initramfs when present
  -> payload.cmdline
~~~

Never put firmware into `payload.kernel` to emulate firmware mode.

Never include `payload.firmware` in direct-kernel mode.

Persist enough logical v1 boot identity for VMM respawn.

Do not make `vm-config.json` the authority for artifact identity if the Core definition is available.

The persisted Cloud Hypervisor payload may remain recovery material, but it must correspond exactly to the authoritative mode and digests.

Add adapter tests that inspect the actual `vm.create` JSON.

Required cases:

- firmware payload has only firmware boot material;
- direct kernel with initramfs;
- direct kernel without initramfs;
- direct kernel command line;
- no mixed payload;
- artifact resolution failure occurs before `vm.create`;
- corrupt cached artifact occurs before `vm.create`;
- no fallback after failure.

### PR F — Restart, recovery, snapshot, and migration semantics

Prove boot identity survives lifecycle recovery.

Cover:

- `chv-agent` restart;
- VMM exit and respawn;
- stopped VM start;
- adopted running VM;
- Core journal replay;
- NodeCache compatibility migration.

A v1 VM must come back with the same:

- boot contract version;
- mode;
- firmware digest or boot-set digest;
- command line.

For snapshot metadata that recreates VM configuration, preserve boot identity.

For migration, add a preflight that proves the destination can resolve the same boot artifacts.

Do not transfer or substitute a different kernel during cutover.

If multi-node boot-artifact distribution does not exist yet, migration of a direct-kernel VM must fail with a clear precondition when the target lacks the required digests.

That is preferable to a false success claim.

### PR G — Real-KVM qualification and measured comparison

Do not edit old evidence.

Create a new qualification scenario and new evidence only after implementation is stable.

Use one supported unmodified Linux cloud image.

Prepare a matching direct-kernel boot set.

Run firmware and direct-kernel mode on the same host and image family.

Record at least:

- API accept to VMM-created;
- start to first console output;
- boot start to Linux kernel banner;
- kernel banner to the userspace-ready marker;
- total start to userspace-ready marker.

Use distributions when practical.

Do not define success as “boots in one second.”

Correctness gates are:

- create;
- start;
- kernel banner;
- userspace-ready marker;
- cloud-init network;
- cloud-init user-data;
- reboot;
- graceful stop;
- start after stop;
- agent restart;
- VMM respawn;
- delete;
- corrupt-artifact failure;
- missing-artifact failure;
- no fallback.

Run the existing firmware qualification path again after the direct-kernel changes.

Firmware regression is a blocker.

Only after this gate passes may living docs call direct kernel boot `QUALIFIED — KVM-VERIFIED` for the tested tuple.

## Data-model guidance

Do not make host paths the new durable identity.

Do not store one arbitrary JSON blob when typed or normalized fields already have an authority boundary.

If the control-plane store needs additive transitional columns, prefer explicit fields such as:

~~~text
boot_contract_version
boot_mode
boot_ref_digest
boot_cmdline
~~~

Existing rows may leave them null during the compatibility window.

The Core domain remains the semantic authority for boot validation.

Avoid two independent boot models that can disagree.

## Artifact-source guidance

The v1 VM definition identifies content. It does not identify a download URL.

Artifact source and distribution belong to the artifact provider or image metadata.

Do not put a remote URL in `VmBootSpecV1` as integrity identity.

If a remote artifact source is added:

- require HTTPS unless the source is explicitly local and operator-controlled;
- verify the digest after download;
- bound size and time;
- use atomic materialization;
- avoid credentials in logs;
- never trust source URL identity instead of digest identity.

## Kernel and initramfs compatibility

Treat the direct-kernel boot set atomically.

Do not select the newest kernel from the root volume at VM start.

Do not select an initramfs by filename similarity alone.

The preparation step must establish the pair.

Record `kernel_release`.

When the root image requires kernel modules, the selected kernel release must match the module tree expected by the image.

Reject an unprovable combination.

Do not hide this problem with a permissive fallback.

## Security requirements

- Validate all digest strings.
- Reject path traversal.
- Reject symlink escape.
- Use atomic final publication.
- Bound artifact size.
- Never execute extraction helpers with user-controlled shell interpolation.
- Mount or inspect source images read-only.
- Clean loop, namespace, or mount resources on every failure path.
- Treat malformed image files as hostile input.
- Do not leak credentials from remote image sources.
- Keep Cloud Hypervisor process ownership unchanged.
- Keep the current fail-closed lifecycle rules.

## Observability requirements

Add bounded boot-mode visibility.

Suitable examples:

~~~text
chv_vm_boot_operations_total{boot_mode="firmware",result="ok"}
chv_vm_boot_operations_total{boot_mode="direct_kernel",result="ok"}
~~~

Do not add per-VM labels.

Logs should include:

- VM ID where existing log policy already allows it;
- boot mode;
- shortened artifact identity for correlation;
- failure class.

Do not log full command-line secrets if a future command line can contain sensitive data.

## Documentation requirements

Update living documentation only after the code changes are true.

Update:

- Cloud Hypervisor reference;
- architecture or deployment architecture where the support matrix lives;
- CLI reference if present;
- image documentation;
- support-state tables.

Do not rewrite ADR-022 after it becomes Accepted to match implementation drift.

If implementation requires a different architectural decision, create a superseding ADR or amend ADR-022 while it is still Proposed.

Do not modify old evidence to make the feature appear qualified.

## Required tests before each merge

Run the smallest relevant package tests during development.

Before each implementation PR is ready for review, run at least:

~~~text
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
~~~

Run the UI checks only when UI code changes.

Run protobuf compatibility checks when protobuf changes.

Run any path-safety tests from a worktree location that satisfies their existing assumptions.

Do not dismiss existing failures. Classify them as introduced, pre-existing, environment-specific, or blocked.

## Forbidden outcomes

Do not merge any implementation that does one of these:

- replaces firmware boot;
- changes the default to direct kernel before qualification;
- infers mode from optional paths in new state;
- stores new arbitrary host paths as VM boot identity;
- mixes kernel and initramfs from different sets;
- starts the VMM before artifact integrity verification;
- silently falls back to firmware;
- patches the root image during VM start;
- mutates the root image to remove initramfs use;
- auto-follows in-VM kernel package updates without an explicit design;
- rewrites old journal history;
- introduces a second VM lifecycle authority;
- introduces another VMM backend;
- upgrades Cloud Hypervisor as a side effect of this feature;
- claims multi-node artifact distribution without implementing and testing it;
- claims real-KVM qualification from unit tests.

## Exit gate

The implementation is complete only when all of these are true:

1. Firmware boot still passes its qualified lifecycle.
2. Direct kernel boot uses an explicit durable mode.
3. Direct kernel boot passes kernel, optional initramfs, and command line to Cloud Hypervisor.
4. New boot identity is content-addressed.
5. Artifact corruption fails before VMM mutation.
6. No silent mode fallback exists.
7. Restart and respawn preserve boot identity.
8. Public APIs do not require host paths.
9. An unmodified supported cloud image passes the direct-kernel real-KVM campaign.
10. The measured firmware-versus-direct comparison is recorded without unsupported performance claims.

At the end, provide a concise implementation report.

The report must include:

- PR sequence and exact commit SHAs;
- schema changes;
- compatibility behavior;
- artifact-store behavior;
- test commands and results;
- real-KVM evidence location;
- measured boot-stage timings;
- remaining unqualified boundaries;
- any follow-up ADR that became necessary.
