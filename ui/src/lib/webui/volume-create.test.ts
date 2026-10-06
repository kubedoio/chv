import { describe, expect, it } from 'vitest';

import {
	MAX_VOLUME_BYTES,
	STORAGE_CLASSES,
	buildCreateVolumePayload,
	gibToCapacityBytes,
	validateCapacityBytes,
	validateStorageClass,
	validateVolumeCreateInput,
	validateVolumeName
} from './volume-create';

describe('volume-create form helpers (#513 DP10)', () => {
	it('mirrors the shared 64 TiB ceiling byte-for-byte', () => {
		// chv_hypervisor_api::resources::MAX_VOLUME_BYTES
		expect(MAX_VOLUME_BYTES).toBe(64 * 1024 * 1024 * 1024 * 1024);
		expect(MAX_VOLUME_BYTES / gibToCapacityBytes(1)).toBe(65536);
	});

	it('converts GiB to capacity_bytes', () => {
		expect(gibToCapacityBytes(1)).toBe(1073741824);
		expect(gibToCapacityBytes(10)).toBe(10 * 1024 * 1024 * 1024);
		expect(gibToCapacityBytes(65536)).toBe(MAX_VOLUME_BYTES);
	});

	describe('validateVolumeName', () => {
		it('accepts the shared display-name vocabulary', () => {
			expect(validateVolumeName('my-data-volume')).toBeNull();
			expect(validateVolumeName('Disk_2.backup')).toBeNull();
			expect(validateVolumeName('  padded  ')).toBeNull();
		});

		it('rejects empty, over-long, and out-of-vocabulary names', () => {
			expect(validateVolumeName('')).toMatch(/required/i);
			expect(validateVolumeName('   ')).toMatch(/required/i);
			expect(validateVolumeName('a'.repeat(65))).toMatch(/1-64/);
			expect(validateVolumeName('bad/name')).toMatch(/1-64/);
			expect(validateVolumeName('bad:name')).toMatch(/1-64/);
		});
	});

	describe('validateCapacityBytes', () => {
		it('accepts the server bounds (1 ..= 64 TiB)', () => {
			expect(validateCapacityBytes(1)).toBeNull();
			expect(validateCapacityBytes(gibToCapacityBytes(1))).toBeNull();
			expect(validateCapacityBytes(MAX_VOLUME_BYTES)).toBeNull();
		});

		it('rejects zero, negative, non-finite, and over-ceiling sizes', () => {
			expect(validateCapacityBytes(0)).toMatch(/1 and 65536 GiB/);
			expect(validateCapacityBytes(-1)).toMatch(/1 and 65536 GiB/);
			expect(validateCapacityBytes(Number.NaN)).toMatch(/1 and 65536 GiB/);
			expect(validateCapacityBytes(MAX_VOLUME_BYTES + 1)).toMatch(/1 and 65536 GiB/);
			// a cleared number input binds as null → 0 bytes
			expect(validateCapacityBytes(gibToCapacityBytes(null as unknown as number))).toMatch(
				/1 and 65536 GiB/
			);
		});
	});

	describe('validateStorageClass', () => {
		it('accepts blank (node default) and the shared vocabulary', () => {
			expect(validateStorageClass('')).toBeNull();
			expect(validateStorageClass('   ')).toBeNull();
			for (const klass of STORAGE_CLASSES) {
				expect(validateStorageClass(klass)).toBeNull();
			}
		});

		it('rejects aliases and case variants, mirroring the server', () => {
			expect(validateStorageClass('LOCAL')).toMatch(/local, iscsi, ceph, lvm/);
			expect(validateStorageClass('localdisk')).toMatch(/local, iscsi, ceph, lvm/);
			expect(validateStorageClass('nfs')).toMatch(/local, iscsi, ceph, lvm/);
		});
	});

	describe('validateVolumeCreateInput', () => {
		it('returns no errors for a complete input', () => {
			expect(
				validateVolumeCreateInput({ name: 'data-1', nodeId: 'node-abc', sizeGib: 10 })
			).toEqual({});
		});

		it('requires a node (presence only — existence is the server\'s story)', () => {
			const errors = validateVolumeCreateInput({ name: 'data-1', nodeId: '  ', sizeGib: 10 });
			expect(errors.nodeId).toMatch(/required/i);
		});
	});

	describe('buildCreateVolumePayload', () => {
		it('sends exactly the DP3 contract keys', () => {
			const payload = buildCreateVolumePayload({
				name: 'data-1',
				nodeId: 'node-abc',
				sizeGib: 10,
				storageClass: 'lvm'
			});
			expect(payload).toEqual({
				name: 'data-1',
				node_id: 'node-abc',
				capacity_bytes: 10 * 1024 * 1024 * 1024,
				storage_class: 'lvm'
			});
			expect(Object.keys(payload).sort()).toEqual(
				['capacity_bytes', 'name', 'node_id', 'storage_class'].sort()
			);
		});

		it('omits storage_class when blank and trims sent values', () => {
			const payload = buildCreateVolumePayload({
				name: '  data-1  ',
				nodeId: ' node-abc ',
				sizeGib: 5,
				storageClass: '   '
			});
			expect(payload).toEqual({
				name: 'data-1',
				node_id: 'node-abc',
				capacity_bytes: 5 * 1024 * 1024 * 1024
			});
			expect('storage_class' in payload).toBe(false);
		});

		it('never expresses the reserved keys (attached_vm_id, seed_image_ref)', () => {
			const payload = buildCreateVolumePayload({
				name: 'data-1',
				nodeId: 'node-abc',
				sizeGib: 10
			});
			expect(payload).not.toHaveProperty('attached_vm_id');
			expect(payload).not.toHaveProperty('seed_image_ref');
		});
	});
});
