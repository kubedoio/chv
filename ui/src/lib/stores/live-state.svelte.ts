import { browser } from '$app/environment';
import { invalidateAll } from '$app/navigation';
import { getStoredToken } from '$lib/api/client';
import { inventory } from './inventory.svelte';
import { invalidatePattern } from './api-cache.svelte';
import { taskStream, type TaskUpdate } from './task-stream.svelte';

export interface InvalidateOpts {
	patterns?: string[];
	sidebar?: boolean;
	detailId?: string;
	delayMs?: number;
}

class LiveState {
	private readonly taskPatternMap: Record<string, string> = {
		CreateVm: 'vms:',
		StartVm: 'vms:',
		ShutdownVm: 'vms:',
		PoweroffVm: 'vms:',
		RestartVm: 'vms:',
		DeleteVm: 'vms:',
		MigrateVm: 'vms:',
		SnapshotVm: 'vms:',
		RestoreSnapshot: 'vms:',
		CreateNode: 'nodes:',
		DeleteNode: 'nodes:',
		CreateNetwork: 'networks:',
		DeleteNetwork: 'networks:',
		CreateVolume: 'volumes:',
		DeleteVolume: 'volumes:',
		ResizeVolume: 'volumes:',
		ImportImage: 'images:',
		DeleteImage: 'images:',
		CreateVmTemplate: 'templates:',
		DeleteVmTemplate: 'templates:',
	};

	private handleTaskCompleted(task: TaskUpdate) {
		const pattern = this.taskPatternMap[task.summary];
		if (!pattern) return;
		this.invalidateAndRefresh({
			patterns: [pattern],
			sidebar: true,
			detailId: task.resource_id,
		});
	}

	constructor() {
		if (browser) {
			taskStream.onTaskCompleted = (task) => this.handleTaskCompleted(task);
		}
	}

	async invalidateAndRefresh(opts: InvalidateOpts = {}) {
		if (!browser) return;

		if (opts.patterns) {
			for (const p of opts.patterns) {
				invalidatePattern(p as import('./api-cache.svelte').CacheKey);
			}
		}

		if (opts.sidebar) {
			await inventory.fetch();
		}

		await invalidateAll();

		if (opts.delayMs && opts.delayMs > 0) {
			setTimeout(() => {
				if (opts.patterns) {
					for (const p of opts.patterns) {
						invalidatePattern(p as import('./api-cache.svelte').CacheKey);
					}
				}
				invalidateAll();
			}, opts.delayMs);
		}
	}
}

export const liveState = new LiveState();
