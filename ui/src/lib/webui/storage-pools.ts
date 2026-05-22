import { listStoragePools } from '$lib/bff/storage';
import type { StoragePoolItem } from '$lib/bff/types';
import type { StoragePool } from '$lib/api/types';

export function mapStoragePoolItem(pool: StoragePoolItem): StoragePool {
	return {
		id: pool.pool_id,
		name: pool.name,
		pool_type: pool.pool_type,
		path: pool.path,
		is_default: pool.is_default,
		status: pool.status,
		capacity_bytes: pool.capacity_bytes,
		allocatable_bytes: pool.allocatable_bytes,
		created_at: pool.created_at
	};
}

export async function loadStoragePoolsFromBff(token?: string): Promise<StoragePool[]> {
	const response = await listStoragePools(token);
	return response.items.map(mapStoragePoolItem);
}
