import { describe, expect, it } from 'vitest';
import {
	buildInstanceActions,
	getInstanceAction,
	normalizeInstanceStatus,
	groupInstancesByHost
} from '$lib/shell/instance-actions';
import type { InstanceStatus, InstanceTreeItem } from '$lib/api/types';

describe('buildInstanceActions', () => {
	it('returns correct actions for a RUNNING instance', () => {
		const actions = buildInstanceActions('running');
		const map = new Map(actions.map((a) => [a.id, a]));

		expect(map.get('open')?.enabled).toBe(true);
		expect(map.get('console')?.enabled).toBe(true);
		expect(map.get('start')?.enabled).toBe(false);
		expect(map.get('start')?.disabledReason).toBe('Already running');
		expect(map.get('shutdown')?.enabled).toBe(true);
		expect(map.get('poweroff')?.enabled).toBe(true);
		expect(map.get('poweroff')?.dangerous).toBe(true);
		expect(map.get('poweroff')?.requiresConfirmation).toBe(true);
		expect(map.get('restart')?.enabled).toBe(true);
		expect(map.get('rename')?.enabled).toBe(false);
		expect(map.get('delete')?.enabled).toBe(true);
		expect(map.get('delete')?.dangerous).toBe(true);
		expect(map.get('delete')?.requiresConfirmation).toBe(true);
	});

	it('returns correct actions for a STOPPED instance', () => {
		const actions = buildInstanceActions('stopped');
		const map = new Map(actions.map((a) => [a.id, a]));

		expect(map.get('open')?.enabled).toBe(true);
		expect(map.get('console')?.enabled).toBe(false);
		expect(map.get('console')?.disabledReason).toBe('Instance is stopped');
		expect(map.get('start')?.enabled).toBe(true);
		expect(map.get('shutdown')?.enabled).toBe(false);
		expect(map.get('shutdown')?.disabledReason).toBe('Instance is stopped');
		expect(map.get('poweroff')?.enabled).toBe(false);
		expect(map.get('poweroff')?.disabledReason).toBe('Instance is stopped');
		expect(map.get('restart')?.enabled).toBe(false);
		expect(map.get('restart')?.disabledReason).toBe('Instance is stopped');
		expect(map.get('rename')?.enabled).toBe(false);
		expect(map.get('delete')?.enabled).toBe(true);
	});

	it('returns correct actions for an ERROR instance', () => {
		const actions = buildInstanceActions('error');
		const map = new Map(actions.map((a) => [a.id, a]));

		expect(map.get('open')?.enabled).toBe(true);
		expect(map.get('console')?.enabled).toBe(false);
		expect(map.get('start')?.enabled).toBe(true);
		expect(map.get('shutdown')?.enabled).toBe(false);
		expect(map.get('poweroff')?.enabled).toBe(true);
		expect(map.get('poweroff')?.dangerous).toBe(true);
		expect(map.get('restart')?.enabled).toBe(false);
		expect(map.get('delete')?.enabled).toBe(true);
	});

	it('returns correct actions for a PAUSED instance', () => {
		const actions = buildInstanceActions('paused');
		const map = new Map(actions.map((a) => [a.id, a]));

		expect(map.get('console')?.enabled).toBe(true);
		expect(map.get('start')?.enabled).toBe(true);
		expect(map.get('shutdown')?.enabled).toBe(true);
		expect(map.get('poweroff')?.enabled).toBe(true);
		expect(map.get('restart')?.enabled).toBe(true);
	});

	it('orders actions consistently', () => {
		const actions = buildInstanceActions('running');
		const ids = actions.map((a) => a.id);
		expect(ids).toEqual([
			'open',
			'console',
			'start',
			'shutdown',
			'poweroff',
			'restart',
			'rename',
			'delete'
		]);
	});
});

describe('getInstanceAction', () => {
	it('finds an existing action', () => {
		const action = getInstanceAction('running', 'delete');
		expect(action).toBeDefined();
		expect(action?.id).toBe('delete');
		expect(action?.dangerous).toBe(true);
	});

	it('returns undefined for unknown action id', () => {
		// @ts-expect-error testing invalid input
		const action = getInstanceAction('running', 'explode');
		expect(action).toBeUndefined();
	});
});

describe('normalizeInstanceStatus', () => {
	it('normalizes running variants', () => {
		expect(normalizeInstanceStatus('running')).toBe('running');
		expect(normalizeInstanceStatus('RUNNING')).toBe('running');
		expect(normalizeInstanceStatus('started')).toBe('running');
		expect(normalizeInstanceStatus('active')).toBe('running');
	});

	it('normalizes stopped variants', () => {
		expect(normalizeInstanceStatus('stopped')).toBe('stopped');
		expect(normalizeInstanceStatus('halted')).toBe('stopped');
		expect(normalizeInstanceStatus('poweredoff')).toBe('stopped');
		expect(normalizeInstanceStatus('powered_off')).toBe('stopped');
	});

	it('normalizes error variants', () => {
		expect(normalizeInstanceStatus('error')).toBe('error');
		expect(normalizeInstanceStatus('failed')).toBe('error');
		expect(normalizeInstanceStatus('crashed')).toBe('error');
	});

	it('returns unknown for unrecognized states', () => {
		expect(normalizeInstanceStatus('migrating')).toBe('unknown');
		expect(normalizeInstanceStatus('')).toBe('unknown');
	});
});

describe('groupInstancesByHost', () => {
	it('groups instances by nodeId', () => {
		const instances: InstanceTreeItem[] = [
			{ id: 'vm-1', name: 'alpha', status: 'running', nodeId: 'node-a' },
			{ id: 'vm-2', name: 'beta', status: 'stopped', nodeId: 'node-b' },
			{ id: 'vm-3', name: 'gamma', status: 'running', nodeId: 'node-a' }
		];

		const grouped = groupInstancesByHost(instances);

		expect(grouped.get('node-a')?.map((i) => i.name)).toEqual(['alpha', 'gamma']);
		expect(grouped.get('node-b')?.map((i) => i.name)).toEqual(['beta']);
	});

	it('returns empty map for empty input', () => {
		const grouped = groupInstancesByHost([]);
		expect(grouped.size).toBe(0);
	});
});
