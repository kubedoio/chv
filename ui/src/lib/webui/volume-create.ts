// Shared constants + validation for the standalone volume-create form
// (#513 DP10). Every rule here mirrors the server's contract on
// POST /v1/volumes/create — the server stays authoritative; these
// client-side mirrors exist so a typo fails locally with the same rule
// instead of a server round-trip (the chvctl `--storage-class`
// discipline).
import type { CreateVolumeRequest } from '$lib/bff/types';

// Mirror of chv_hypervisor_api::resources::MAX_VOLUME_BYTES (64 TiB) —
// the single Rust-side constant of record (consolidated in #513 PR 3).
// The UI cannot import the Rust const, so this local mirror carries the
// cross-reference; changing the ceiling is a two-site edit, and the
// test below pins the value.
export const MAX_VOLUME_BYTES = 64 * 1024 * 1024 * 1024 * 1024;

export const GIB_BYTES = 1024 * 1024 * 1024;

// Mirror of the server's shared display-name guard
// (`is_valid_display_name` in the BFF: ^[A-Za-z0-9 ._-]{1,64}$ — the
// name is interpolated into volume names/paths).
export const VOLUME_NAME_PATTERN = /^[A-Za-z0-9 ._-]{1,64}$/;

// Mirror of chv_hypervisor_api::resources::BACKEND_CLASSES — the one
// shared vocabulary the BFF validates against (#379 DP3 / #513 DP5).
export const STORAGE_CLASSES = ['local', 'iscsi', 'ceph', 'lvm'] as const;
export type StorageClass = (typeof STORAGE_CLASSES)[number];

// The form is GiB-denominated (the sibling "Disk Size (GB)" convention);
// the wire field is `capacity_bytes`.
export function gibToCapacityBytes(sizeGib: number): number {
	return Math.round(sizeGib * GIB_BYTES);
}

export function validateVolumeName(name: string): string | null {
	const trimmed = name.trim();
	if (trimmed === '') {
		return 'Name is required';
	}
	if (!VOLUME_NAME_PATTERN.test(trimmed)) {
		return 'Name must be 1-64 characters (letters, numbers, spaces, dots, underscores, hyphens)';
	}
	return null;
}

export function validateCapacityBytes(capacityBytes: number): string | null {
	if (!Number.isFinite(capacityBytes) || capacityBytes <= 0 || capacityBytes > MAX_VOLUME_BYTES) {
		return 'Size must be between 1 and 65536 GiB (64 TiB)';
	}
	return null;
}

// Blank/absent means the node's default (local) — same convention as
// the server's blank-storage_class handling. Aliases and case variants
// are rejected, mirroring the server's vocabulary check.
export function validateStorageClass(storageClass: string): string | null {
	const trimmed = storageClass.trim();
	if (trimmed === '') {
		return null;
	}
	if (!(STORAGE_CLASSES as readonly string[]).includes(trimmed)) {
		return `Storage class must be one of: ${STORAGE_CLASSES.join(', ')}`;
	}
	return null;
}

export interface VolumeCreateInput {
	name: string;
	nodeId: string;
	sizeGib: number;
	storageClass?: string;
}

export type VolumeCreateFieldErrors = {
	name?: string;
	nodeId?: string;
	size?: string;
	storageClass?: string;
};

export function validateVolumeCreateInput(input: VolumeCreateInput): VolumeCreateFieldErrors {
	const errors: VolumeCreateFieldErrors = {};
	const nameError = validateVolumeName(input.name);
	if (nameError) errors.name = nameError;
	// `node_id` is required — standalone volumes have no VM to place
	// them and the route has no default placement node (DP3). This is a
	// presence check only: node EXISTENCE is never gated client-side
	// (fail-open — the server accepts and the task surfaces the
	// failure).
	if (input.nodeId.trim() === '') {
		errors.nodeId = 'Node is required';
	}
	const capacityError = validateCapacityBytes(gibToCapacityBytes(input.sizeGib));
	if (capacityError) errors.size = capacityError;
	const classError = validateStorageClass(input.storageClass ?? '');
	if (classError) errors.storageClass = classError;
	return errors;
}

// Builds the wire payload: exactly the DP3 contract keys. The reserved
// keys (`attached_vm_id`, `seed_image_ref`) are never expressible —
// they exist nowhere in this module or the form, so the payload cannot
// carry them (the BFF's loud 400s are the API-user story, not ours).
export function buildCreateVolumePayload(input: VolumeCreateInput): CreateVolumeRequest {
	const payload: CreateVolumeRequest = {
		name: input.name.trim(),
		node_id: input.nodeId.trim(),
		capacity_bytes: gibToCapacityBytes(input.sizeGib)
	};
	const storageClass = (input.storageClass ?? '').trim();
	if (storageClass !== '') {
		payload.storage_class = storageClass;
	}
	return payload;
}
