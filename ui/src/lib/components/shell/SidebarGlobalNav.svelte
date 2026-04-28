<script lang="ts">
	import {
		Image,
		Network,
		HardDrive,
		Activity,
		AlertCircle,
		ShieldCheck,
		Settings
	} from 'lucide-svelte';

	interface Props {
		pathname: string;
		isActive: (href: string, pathname: string) => boolean;
	}

	let { pathname, isActive }: Props = $props();
</script>

<div class="flex flex-col gap-1">
	<div class="text-[10px] font-bold uppercase text-[var(--color-neutral-500)] mb-1 pl-2 tracking-wider">Global</div>

	<a
		href="/images"
		class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/images', pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
		aria-current={isActive('/images', pathname) ? 'page' : undefined}
	>
		<Image size={14} />
		<span>Images</span>
	</a>

	<a
		href="/networks"
		class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/networks', pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
		aria-current={isActive('/networks', pathname) ? 'page' : undefined}
	>
		<Network size={14} />
		<span>Networks</span>
	</a>

	<a
		href="/storage"
		class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/storage', pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
		aria-current={isActive('/storage', pathname) ? 'page' : undefined}
	>
		<HardDrive size={14} />
		<span>Storage Pools</span>
	</a>

	<a
		href="/tasks"
		class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/tasks', pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
		aria-current={isActive('/tasks', pathname) ? 'page' : undefined}
	>
		<Activity size={14} />
		<span>Tasks</span>
	</a>

	<a
		href="/events"
		class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/events', pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
		aria-current={isActive('/events', pathname) ? 'page' : undefined}
	>
		<AlertCircle size={14} />
		<span>Events</span>
	</a>

	<a
		href="/backup-jobs"
		class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/backup-jobs', pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
		aria-current={isActive('/backup-jobs', pathname) ? 'page' : undefined}
	>
		<ShieldCheck size={14} />
		<span>Backups</span>
	</a>

	<a
		href="/settings"
		class="flex items-center gap-[0.625rem] py-[0.35rem] px-2 text-[length:var(--text-sm)] text-[var(--color-neutral-300)] no-underline rounded-[var(--radius-xs)] transition-all duration-[120ms] ease-in-out hover:bg-[var(--color-neutral-800)] hover:text-[var(--color-sidebar-text-active,#ffffff)] {isActive('/settings', pathname) ? 'bg-[var(--color-primary)] text-[var(--color-sidebar-text-active,#ffffff)]' : ''}"
		aria-current={isActive('/settings', pathname) ? 'page' : undefined}
	>
		<Settings size={14} />
		<span>Settings</span>
	</a>
</div>
