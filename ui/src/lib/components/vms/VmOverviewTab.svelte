<script lang="ts">
	import VmDetailSummaryTab from '$lib/components/vms/VmDetailSummaryTab.svelte';

	interface Props {
		powerState: string;
		health: string;
		cpu: string;
		memory: string;
		volumes?: any[];
		nics?: any[];
		recentTasks?: any[];
	}

	let {
		powerState,
		health,
		cpu,
		memory,
		volumes = [],
		nics = [],
		recentTasks = []
	}: Props = $props();
</script>

<VmDetailSummaryTab
	{powerState}
	{health}
	{cpu}
	{memory}
	{volumes}
	{nics}
	{recentTasks}
/>
