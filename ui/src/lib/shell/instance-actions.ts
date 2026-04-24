import type {
	InstanceActionId,
	InstanceActionDefinition,
	InstanceStatus
} from '$lib/api/types';

/**
 * Build the complete list of instance actions with availability
 * determined by the instance's current power state.
 *
 * @param status - current instance status
 * @returns ordered list of action definitions
 */
export function buildInstanceActions(status: InstanceStatus): InstanceActionDefinition[] {
	const s = status.toLowerCase() as InstanceStatus;

	const isRunning = s === 'running';
	const isStopped = s === 'stopped';
	const isError = s === 'error';
	const isPaused = s === 'paused';

	return [
		{
			id: 'open',
			label: 'Open',
			enabled: true,
			dangerous: false,
			requiresConfirmation: false
		},
		{
			id: 'console',
			label: 'Console',
			enabled: isRunning || isPaused,
			dangerous: false,
			requiresConfirmation: false,
			disabledReason: isStopped || isError ? 'Instance is stopped' : undefined
		},
		{
			id: 'start',
			label: 'Start',
			enabled: isStopped || isError || isPaused,
			dangerous: false,
			requiresConfirmation: false,
			disabledReason: isRunning ? 'Already running' : undefined
		},
		{
			id: 'shutdown',
			label: 'Shutdown',
			enabled: isRunning || isPaused,
			dangerous: false,
			requiresConfirmation: false,
			disabledReason: isStopped || isError ? 'Instance is stopped' : undefined
		},
		{
			id: 'poweroff',
			label: 'Power Off',
			enabled: isRunning || isPaused || isError,
			dangerous: true,
			requiresConfirmation: true,
			disabledReason: isStopped ? 'Instance is stopped' : undefined
		},
		{
			id: 'restart',
			label: 'Restart',
			enabled: isRunning || isPaused,
			dangerous: false,
			requiresConfirmation: false,
			disabledReason: isStopped || isError ? 'Instance is stopped' : undefined
		},
		{
			id: 'rename',
			label: 'Rename',
			enabled: false,
			dangerous: false,
			requiresConfirmation: false,
			disabledReason: 'Not yet supported'
		},
		{
			id: 'delete',
			label: 'Delete',
			enabled: true,
			dangerous: true,
			requiresConfirmation: true
		}
	];
}

/**
 * Get a single action definition by ID for a given status.
 */
export function getInstanceAction(
	status: InstanceStatus,
	actionId: InstanceActionId
): InstanceActionDefinition | undefined {
	return buildInstanceActions(status).find((a) => a.id === actionId);
}

/**
 * Normalize a raw power-state string into the canonical InstanceStatus.
 */
export function normalizeInstanceStatus(raw: string): InstanceStatus {
	const s = raw.toLowerCase().trim();
	if (s === 'running' || s === 'started' || s === 'active') return 'running';
	if (s === 'stopped' || s === 'halted' || s === 'poweredoff' || s === 'powered_off') return 'stopped';
	if (s === 'error' || s === 'failed' || s === 'crashed') return 'error';
	if (s === 'paused') return 'paused';
	return 'unknown';
}

/**
 * Group a flat list of instances by their host (node) ID.
 */
export function groupInstancesByHost<T extends { nodeId: string; id: string; name: string; status: InstanceStatus }>(
	instances: T[]
): Map<string, T[]> {
	const map = new Map<string, T[]>();
	for (const inst of instances) {
		const list = map.get(inst.nodeId) ?? [];
		list.push(inst);
		map.set(inst.nodeId, list);
	}
	return map;
}
