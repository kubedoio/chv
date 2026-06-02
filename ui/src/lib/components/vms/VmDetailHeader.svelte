<script lang="ts">
	import ResourceDetailHeader from '$lib/components/shell/ResourceDetailHeader.svelte';
	import VmDetailActions from '$lib/components/vms/VmDetailActions.svelte';
	import type { ShellTone } from '$lib/shell/app-shell';

	interface Props {
		title: string;
		eyebrow: string;
		statusLabel: string;
		tone: ShellTone;
		parentLabel: string;
		parentHref: string;
		pendingAction: string | null;
		powerState: string;
		onExecute: (action: string) => void;
		onMigrate: () => void;
	}

	let {
		title,
		eyebrow,
		statusLabel,
		tone,
		parentLabel,
		parentHref,
		pendingAction,
		powerState,
		onExecute,
		onMigrate
	}: Props = $props();
</script>

<ResourceDetailHeader
	{title}
	{eyebrow}
	{statusLabel}
	{tone}
	{parentLabel}
	{parentHref}
>
	{#snippet actions()}
		<VmDetailActions {pendingAction} {powerState} onExecute={onExecute} onMigrate={onMigrate} />
	{/snippet}
</ResourceDetailHeader>
