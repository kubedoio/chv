import { describe, expect, it } from 'vitest';

import type { Event, Network, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import {
	buildNodeDetail,
	buildNodesList,
	buildVmDetail,
	buildVmsList
} from '$lib/webui/resources';

const nodes: NodeWithResources[] = [
	{
		id: 'node-1',
		name: 'ber-01',
		hostname: 'ber-01.local',
		ip_address: '10.0.0.11',
		status: 'online',
		is_local: true,
		resources: {
			vms: 2,
			images: 1,
			storage_pools: 2,
			networks: 2
		}
	},
	{
		id: 'node-2',
		name: 'ber-02',
		hostname: 'ber-02.local',
		ip_address: '10.0.0.12',
		status: 'maintenance',
		is_local: false,
		resources: {
			vms: 1,
			images: 1,
			storage_pools: 1,
			networks: 1
		}
	}
];

const vms: VM[] = [
	{
		id: 'vm-1',
		name: 'api-01',
		image_id: 'img-1',
		storage_pool_id: 'pool-1',
		network_id: 'net-1',
		desired_state: 'running',
		actual_state: 'running',
		vcpu: 2,
		memory_mb: 4096,
		disk_path: '/var/lib/chv/api-01.qcow2',
		seed_iso_path: '/var/lib/chv/api-01-seed.iso',
		workspace_path: '/workspaces/api-01',
		ip_address: '192.168.10.20'
	},
	{
		id: 'vm-2',
		name: 'jobs-01',
		image_id: 'img-2',
		storage_pool_id: 'pool-2',
		network_id: 'net-2',
		desired_state: 'running',
		actual_state: 'failed',
		vcpu: 4,
		memory_mb: 8192,
		disk_path: '/var/lib/chv/jobs-01.qcow2',
		seed_iso_path: '/var/lib/chv/jobs-01-seed.iso',
		workspace_path: '/workspaces/jobs-01',
		last_error: 'Guest never reached ready state'
	},
	{
		id: 'vm-3',
		name: 'batch-01',
		image_id: 'img-2',
		storage_pool_id: 'pool-2',
		network_id: 'net-2',
		desired_state: 'stopped',
		actual_state: 'stopped',
		vcpu: 1,
		memory_mb: 2048,
		disk_path: '/var/lib/chv/batch-01.qcow2',
		seed_iso_path: '/var/lib/chv/batch-01-seed.iso',
		workspace_path: '/workspaces/batch-01'
	}
];

const pools: StoragePool[] = [
	{
		id: 'pool-1',
		name: 'fast',
		pool_type: 'localdisk',
		path: '/var/lib/chv/fast',
		is_default: true,
		status: 'ready',
		capacity_bytes: 500_000_000_000,
		allocatable_bytes: 200_000_000_000,
		created_at: '2026-04-01T00:00:00.000Z'
	},
	{
		id: 'pool-2',
		name: 'bulk',
		pool_type: 'localdisk',
		path: '/var/lib/chv/bulk',
		is_default: false,
		status: 'degraded',
		capacity_bytes: 1_000_000_000_000,
		allocatable_bytes: 300_000_000_000,
		created_at: '2026-04-01T00:00:00.000Z'
	}
];

const networks: Network[] = [
	{
		id: 'net-1',
		name: 'prod',
		mode: 'bridge',
		bridge_name: 'br0',
		cidr: '192.168.10.0/24',
		gateway_ip: '192.168.10.1',
		is_system_managed: true,
		status: 'ready',
		created_at: '2026-04-01T00:00:00.000Z'
	},
	{
		id: 'net-2',
		name: 'stage',
		mode: 'bridge',
		bridge_name: 'br1',
		cidr: '192.168.20.0/24',
		gateway_ip: '192.168.20.1',
		is_system_managed: false,
		status: 'degraded',
		created_at: '2026-04-01T00:00:00.000Z'
	}
];

const operations: Operation[] = [
	{
		id: 'task-node-maint',
		resource_type: 'node',
		resource_id: 'node-2',
		operation_type: 'maintenance',
		state: 'running',
		created_at: '2026-04-15T09:00:00.000Z'
	},
	{
		id: 'task-vm-start',
		resource_type: 'vm',
		resource_id: 'vm-1',
		operation_type: 'start',
		state: 'succeeded',
		created_at: '2026-04-15T10:00:00.000Z'
	},
	{
		id: 'task-vm-restart',
		resource_type: 'vm',
		resource_id: 'vm-2',
		operation_type: 'restart',
		state: 'failed',
		created_at: '2026-04-15T11:00:00.000Z'
	}
];

const events: Event[] = [
	{
		id: 'event-node',
		timestamp: '2026-04-15T09:05:00.000Z',
		operation: 'maintenance',
		status: 'pending',
		resource: 'node',
		resource_id: 'node-2',
		message: 'Maintenance drain still in progress'
	},
	{
		id: 'event-vm',
		timestamp: '2026-04-15T11:02:00.000Z',
		operation: 'restart',
		status: 'failed',
		resource: 'vm',
		resource_id: 'vm-2',
		message: 'Guest agent did not respond before timeout'
	}
];

describe('node and VM list view models', () => {
	it('builds a filterable node list with explicit state and maintenance rendering', () => {
		const model = buildNodesList(
			{ nodes, operations, events },
			{ query: 'ber', maintenance: 'true' },
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(model.items).toHaveLength(1);
		expect(model.items[0]).toMatchObject({
			nodeId: 'node-2',
			healthLabel: 'Maintenance',
			maintenanceLabel: 'In maintenance'
		});
		expect(model.filters.applied).toEqual({
			query: 'ber',
			maintenance: 'true'
		});
	});

	it('builds a filterable VM list with health and last-task context', () => {
		const model = buildVmsList(
			{ vms, nodes, operations, events },
			{ powerState: 'failed', query: 'jobs' },
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(model.items).toHaveLength(1);
		expect(model.items[0]).toMatchObject({
			vmId: 'vm-2',
			healthLabel: 'Failed',
			lastTaskLabel: 'Failed'
		});
	});
});

describe('node and VM detail view models', () => {
	it('builds a node detail model with related tasks and clear sections', () => {
		const detail = buildNodeDetail(
			{
				nodes,
				nodeVms: [vms[2]],
				nodeStoragePools: [pools[1]],
				nodeNetworks: [networks[1]],
				operations,
				events
			},
			'node-2',
			'tasks',
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(detail.state).toBe('ready');
		expect(detail.currentTab).toBe('tasks');
		expect(detail.recentTasks[0]).toMatchObject({
			taskId: 'task-node-maint',
			resourceId: 'node-2'
		});
		expect(detail.sections.map((section) => section.id)).toEqual([
			'summary',
			'vms',
			'volumes',
			'networks',
			'tasks',
			'events',
			'configuration'
		]);
	});

	it('renders healthy storage pools as healthy instead of unknown', () => {
		const detail = buildNodeDetail(
			{
				nodes,
				nodeVms: [vms[0]],
				nodeStoragePools: [pools[0]],
				nodeNetworks: [networks[0]],
				operations,
				events
			},
			'node-1',
			'summary',
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(detail.summaryCards.find((card) => card.label === 'Storage pools')?.tone).toBe('healthy');
	});

	it('builds a VM detail model with related tasks and filtered events', () => {
		const detail = buildVmDetail(
			{
				vm: vms[1],
				nodes,
				storagePools: pools,
				networks,
				operations,
				events
			},
			'tasks',
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(detail.state).toBe('ready');
		expect(detail.currentTab).toBe('tasks');
		expect(detail.recentTasks[0]).toMatchObject({
			taskId: 'task-vm-restart',
			failureReason: 'Guest agent did not respond before timeout'
		});
		expect(detail.events[0].message).toBe('Guest agent did not respond before timeout');
	});
});
