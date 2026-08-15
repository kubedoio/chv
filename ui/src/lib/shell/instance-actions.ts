import type {
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
