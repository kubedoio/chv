<script>
	import StatsCard from '$lib/components/StatsCard.svelte';
	import { Server, Play, Square, Activity } from 'lucide-svelte';
</script>

<div class="p-6">
	<h1 class="mb-6 text-xl font-semibold text-ink">Stats Card Test</h1>

	<!-- Desktop: 4 columns -->
	<div class="mb-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
		<StatsCard title="Total VMs" value={12} icon={Server} />
		<StatsCard title="Running" value={8} icon={Play} trend="up" />
		<StatsCard title="Stopped" value={4} icon={Square} trend="neutral" />
		<StatsCard title="CPU Usage" value="64%" icon={Activity} trend="down" />
	</div>

	<!-- Without icons -->
	<h2 class="mb-4 text-lg font-medium text-muted">Without Icons</h2>
	<div class="mb-8 grid grid-cols-1 gap-4 sm:grid-cols-3">
		<StatsCard title="Memory Used" value="8.4 GB" trend="up" />
		<StatsCard title="Disk I/O" value="124 MB/s" />
		<StatsCard title="Network" value="2.1 Gbps" trend="down" />
	</div>

	<!-- Single card examples -->
	<h2 class="mb-4 text-lg font-medium text-muted">Single Cards</h2>
	<div class="space-y-4">
		<StatsCard title="Just a value" value={42} />
		<StatsCard title="With icon only" value={99} icon={Server} />
		<StatsCard title="With trend only" value={50} trend="up" />
	</div>
</div>
