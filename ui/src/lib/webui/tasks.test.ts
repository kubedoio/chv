import { describe, expect, it } from 'vitest';

import type { Event, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import { buildOverviewModel } from '$lib/webui/overview';
import { buildTaskList, getTaskStatusMeta } from '$lib/webui/tasks';

const operations: Operation[] = [
	{
		id: 'task-queued',
		resource_type: 'vm',
		resource_id: 'vm-01',
		operation_type: 'start',
		state: 'queued',
		created_at: '2026-04-15T10:00:00.000Z'
	},
	{
		id: 'task-running',
		resource_type: 'node',
		resource_id: 'node-02',
		operation_type: 'drain',
		state: 'running',
		created_at: '2026-04-15T09:00:00.000Z'
	},
	{
		id: 'task-failed',
		resource_type: 'volume',
		resource_id: 'pool-fast',
		operation_type: 'expand',
		state: 'failed',
		created_at: '2026-04-14T08:00:00.000Z'
	},
	{
		id: 'task-succeeded',
		resource_type: 'vm',
		resource_id: 'vm-02',
		operation_type: 'restart',
		state: 'succeeded',
		created_at: '2026-04-10T08:00:00.000Z'
	}
];

const events: Event[] = [
	{
		id: 'event-1',
		timestamp: '2026-04-14T08:12:00.000Z',
		operation: 'expand',
		status: 'failed',
		resource: 'volume',
		resource_id: 'pool-fast',
		message: 'Expansion blocked by thin pool threshold'
	},
	{
		id: 'event-2',
		timestamp: '2026-04-10T08:08:00.000Z',
		operation: 'restart',
		status: 'success',
		resource: 'vm',
		resource_id: 'vm-02',
		message: 'Guest came back online'
	}
];

const nodes: NodeWithResources[] = [
	{
		id: 'node-01',
		name: 'ber-01',
		hostname: 'ber-01.local',
		ip_address: '10.0.0.11',
		status: 'online',
		is_local: true,
		resources: {
			vms: 4,
			images: 2,
			storage_pools: 1,
			networks: 2
		}
	},
	{
		id: 'node-02',
		name: 'ber-02',
		hostname: 'ber-02.local',
		ip_address: '10.0.0.12',
		status: 'error',
		is_local: false,
		resources: {
			vms: 2,
			images: 2,
			storage_pools: 1,
			networks: 2
		}
	}
];

const vms: VM[] = [
	{
		id: 'vm-01',
		name: 'api-01',
		image_id: 'img-01',
		storage_pool_id: 'pool-fast',
		network_id: 'net-prod',
		desired_state: 'running',
		actual_state: 'running',
		vcpu: 2,
		memory_mb: 4096,
		disk_path: '/data/api-01.qcow2',
		seed_iso_path: '/data/api-01-seed.iso',
		workspace_path: '/workspaces/api-01'
	},
	{
		id: 'vm-02',
		name: 'jobs-01',
		image_id: 'img-02',
		storage_pool_id: 'pool-fast',
		network_id: 'net-prod',
		desired_state: 'running',
		actual_state: 'failed',
		vcpu: 4,
		memory_mb: 8192,
		disk_path: '/data/jobs-01.qcow2',
		seed_iso_path: '/data/jobs-01-seed.iso',
		workspace_path: '/workspaces/jobs-01',
		last_error: 'Boot sequence stalled'
	},
	{
		id: 'vm-03',
		name: 'batch-01',
		image_id: 'img-03',
		storage_pool_id: 'pool-slow',
		network_id: 'net-stage',
		desired_state: 'stopped',
		actual_state: 'unknown',
		vcpu: 2,
		memory_mb: 2048,
		disk_path: '/data/batch-01.qcow2',
		seed_iso_path: '/data/batch-01-seed.iso',
		workspace_path: '/workspaces/batch-01'
	}
];

const storagePools: StoragePool[] = [
	{
		id: 'pool-fast',
		name: 'fast',
		pool_type: 'localdisk',
		path: '/var/lib/chv/fast',
		is_default: true,
		status: 'ready',
		capacity_bytes: 1_000_000_000_000,
		allocatable_bytes: 250_000_000_000,
		created_at: '2026-04-01T00:00:00.000Z'
	},
	{
		id: 'pool-slow',
		name: 'slow',
		pool_type: 'localdisk',
		path: '/var/lib/chv/slow',
		is_default: false,
		status: 'degraded',
		capacity_bytes: 500_000_000_000,
		allocatable_bytes: 100_000_000_000,
		created_at: '2026-04-01T00:00:00.000Z'
	}
];

describe('task view models', () => {
	it('keeps accepted, in-progress, and completed task states distinct', () => {
		expect(getTaskStatusMeta('queued')).toMatchObject({
			key: 'queued',
			label: 'Accepted'
		});
		expect(getTaskStatusMeta('running')).toMatchObject({
			key: 'running',
			label: 'In progress'
		});
		expect(getTaskStatusMeta('succeeded')).toMatchObject({
			key: 'succeeded',
			label: 'Completed'
		});
	});

	it('maps legacy task state spellings into the explicit UI state model', () => {
		expect(getTaskStatusMeta('success').key).toBe('succeeded');
		expect(getTaskStatusMeta('pending').key).toBe('queued');
		expect(getTaskStatusMeta('mystery').key).toBe('unknown');
	});

	it('filters tasks by status, resource kind, query, and time window', () => {
		const taskPage = buildTaskList(
			{ operations, events },
			{
				status: 'failed',
				resourceKind: 'volume',
				query: 'expand',
				window: '7d'
			},
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(taskPage.items).toHaveLength(1);
		expect(taskPage.items[0]).toMatchObject({
			taskId: 'task-failed',
			resourceKind: 'volume',
			failureReason: 'Expansion blocked by thin pool threshold'
		});
		expect(taskPage.filters.applied).toEqual({
			status: 'failed',
			resourceKind: 'volume',
			query: 'expand',
			window: '7d'
		});
	});

	it('returns explicit task page states when nothing matches or data failed entirely', () => {
		const emptyPage = buildTaskList(
			{ operations, events },
			{ status: 'cancelled' },
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);
		const errorPage = buildTaskList(
			{ operations: [], events: [] },
			{},
			{ fetchFailed: true, now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(emptyPage.state).toBe('empty');
		expect(errorPage.state).toBe('error');
	});

	it('treats task-source outages as errors instead of empty history', () => {
		const taskPage = buildTaskList(
			{ operations: [], events },
			{},
			{ primaryDataUnavailable: true, now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(taskPage.state).toBe('error');
	});

	it('keeps recent-history defaults visible without marking them as applied filters', () => {
		const taskPage = buildTaskList(
			{ operations, events },
			{},
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(taskPage.filters.current).toEqual({
			status: 'all',
			resourceKind: 'all',
			query: '',
			window: '7d'
		});
		expect(taskPage.filters.applied).toEqual({});
	});
});

describe('overview view models', () => {
	it('shapes health, capacity, alerts, and recent tasks for the overview page', () => {
		const overview = buildOverviewModel(
			{ nodes, vms, storagePools, operations, events },
			{ now: new Date('2026-04-15T12:00:00.000Z') }
		);

		expect(overview.state).toBe('ready');
		expect(overview.healthTiles).toHaveLength(4);
		expect(overview.capacityTiles).toHaveLength(3);
		expect(overview.healthTiles.map((tile) => tile.status)).toEqual(
			expect.arrayContaining(['failed', 'degraded'])
		);
		expect(overview.activeAlerts).toEqual(
			expect.arrayContaining([
				'1 node reporting errors',
				'1 VM failed to reach its desired state',
				'Expansion blocked by thin pool threshold'
			])
		);
		expect(overview.recentTasks[0]).toMatchObject({
			taskId: 'task-queued',
			label: 'Accepted'
		});
	});
});
