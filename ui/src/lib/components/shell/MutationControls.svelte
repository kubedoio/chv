<script lang="ts">
	import { Activity, Terminal, Settings } from 'lucide-svelte';
</script>

<div class="grid grid-cols-3 gap-2">
	<button class="flex flex-col items-center gap-2 px-2 py-3 bg-[var(--bg-surface)] border border-[var(--border-subtle)] rounded-[var(--radius-xs)] text-[9px] font-extrabold text-[var(--color-neutral-600)] cursor-pointer transition-all duration-150 ease-in-out disabled:opacity-50 disabled:cursor-not-allowed hover:[&:not(:disabled)]:bg-[var(--bg-surface-muted)] hover:[&:not(:disabled)]:border-[var(--color-primary)] hover:[&:not(:disabled)]:text-[var(--color-primary)]" disabled title="Coming soon">
		<Activity size={14} />
		<span>SYNC</span>
	</button>
	<button class="flex flex-col items-center gap-2 px-2 py-3 bg-[var(--bg-surface)] border border-[var(--border-subtle)] rounded-[var(--radius-xs)] text-[9px] font-extrabold text-[var(--color-neutral-600)] cursor-pointer transition-all duration-150 ease-in-out disabled:opacity-50 disabled:cursor-not-allowed hover:[&:not(:disabled)]:bg-[var(--bg-surface-muted)] hover:[&:not(:disabled)]:border-[var(--color-primary)] hover:[&:not(:disabled)]:text-[var(--color-primary)]" disabled title="Coming soon">
		<Terminal size={14} />
		<span>BYPASS_SHELL</span>
	</button>
	<button class="flex flex-col items-center gap-2 px-2 py-3 bg-[var(--bg-surface)] border border-[var(--border-subtle)] rounded-[var(--radius-xs)] text-[9px] font-extrabold text-[var(--color-neutral-600)] cursor-pointer transition-all duration-150 ease-in-out disabled:opacity-50 disabled:cursor-not-allowed hover:[&:not(:disabled)]:bg-[var(--bg-surface-muted)] hover:[&:not(:disabled)]:border-[var(--color-primary)] hover:[&:not(:disabled)]:text-[var(--color-primary)]" disabled title="Coming soon">
		<Settings size={14} />
		<span>CONFIG</span>
	</button>
</div>
