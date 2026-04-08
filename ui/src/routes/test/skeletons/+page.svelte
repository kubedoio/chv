<script lang="ts">
	import SkeletonRow from '$lib/components/SkeletonRow.svelte';
	import SkeletonCard from '$lib/components/SkeletonCard.svelte';

	let tableLoading = $state(true);
	let cardLoading = $state(true);
	let customColumns = $state(6);
	let customLines = $state(4);
</script>

<div class="space-y-8 p-6">
	<h1 class="text-2xl font-semibold text-ink">Skeleton Loading States Test</h1>

	<!-- SkeletonRow Test -->
	<section class="table-card">
		<div class="card-header px-4 py-3">
			<div class="flex items-center justify-between">
				<div>
					<div class="text-[11px] uppercase tracking-[0.16em] text-muted">Component Test</div>
					<div class="mt-1 text-lg font-semibold">SkeletonRow</div>
				</div>
				<button
					onclick={() => (tableLoading = !tableLoading)}
					class="button-primary rounded px-4 py-2 text-sm"
				>
					{tableLoading ? 'Show Data' : 'Show Skeleton'}
				</button>
			</div>
		</div>

		<div class="p-4">
			<div class="mb-4 flex items-center gap-4">
				<label class="text-sm text-muted">
					Columns:
					<input
						type="number"
						min="1"
						max="10"
						bind:value={customColumns}
						class="ml-2 h-8 w-16 rounded border border-line px-2 text-sm"
					/>
				</label>
			</div>

			<table class="w-full border-collapse text-sm">
				<thead class="bg-chrome text-left uppercase tracking-[0.08em] text-muted">
					<tr>
						<th class="border-b border-line px-4 py-3">Column 1</th>
						<th class="border-b border-line px-4 py-3">Column 2</th>
						<th class="border-b border-line px-4 py-3">Column 3</th>
						<th class="border-b border-line px-4 py-3">Column 4</th>
						<th class="border-b border-line px-4 py-3">Column 5</th>
						<th class="border-b border-line px-4 py-3">Status</th>
					</tr>
				</thead>
				<tbody>
					{#if tableLoading}
						{#each Array(5) as _}
							<SkeletonRow columns={customColumns} />
						{/each}
					{:else}
						{#each Array(5) as _, i}
							<tr class="odd:bg-white even:bg-[#f8f8f8]">
								<td class="border-b border-line px-4 py-3">Item {i + 1}</td>
								<td class="border-b border-line px-4 py-3">Value {i + 1}</td>
								<td class="border-b border-line px-4 py-3">Data {i + 1}</td>
								<td class="border-b border-line px-4 py-3">Info {i + 1}</td>
								<td class="border-b border-line px-4 py-3">Type {i + 1}</td>
								<td class="border-b border-line px-4 py-3">Active</td>
							</tr>
						{/each}
					{/if}
				</tbody>
			</table>
		</div>
	</section>

	<!-- SkeletonCard Test -->
	<section class="table-card">
		<div class="card-header px-4 py-3">
			<div class="flex items-center justify-between">
				<div>
					<div class="text-[11px] uppercase tracking-[0.16em] text-muted">Component Test</div>
					<div class="mt-1 text-lg font-semibold">SkeletonCard</div>
				</div>
				<button
					onclick={() => (cardLoading = !cardLoading)}
					class="button-primary rounded px-4 py-2 text-sm"
				>
					{cardLoading ? 'Show Data' : 'Show Skeleton'}
				</button>
			</div>
		</div>

		<div class="p-4">
			<div class="mb-4 flex items-center gap-4">
				<label class="text-sm text-muted">
					Lines:
					<input
						type="number"
						min="1"
						max="8"
						bind:value={customLines}
						class="ml-2 h-8 w-16 rounded border border-line px-2 text-sm"
					/>
				</label>
			</div>

			<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
				{#if cardLoading}
					{#each Array(6) as _}
						<SkeletonCard lines={customLines} />
					{/each}
				{:else}
					{#each Array(6) as _, i}
						<div class="rounded border border-line bg-white p-4">
							<div class="flex items-start gap-4">
								<div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary">
									{i + 1}
								</div>
								<div class="flex-1">
									<h3 class="font-medium text-ink">Card Title {i + 1}</h3>
									<p class="mt-1 text-sm text-muted">Description line one for this card item.</p>
									<p class="mt-1 text-sm text-muted">Additional info here.</p>
								</div>
							</div>
						</div>
					{/each}
				{/if}
			</div>
		</div>
	</section>

	<!-- Documentation -->
	<section class="rounded border border-line bg-chrome/50 p-4">
		<h2 class="mb-4 text-lg font-semibold text-ink">Usage</h2>
		<pre class="overflow-x-auto rounded bg-white p-4 text-sm mono">
&lt;!-- Table loading state --&gt;
{'{#if loading}'}
  {'{#each Array(5) as _}'}
    &lt;SkeletonRow columns={'{6}'} /&gt;
  {'{/each}'}
{'{:else}'}
  {'{#each items as item}'}...
{'{/if}'}

&lt;!-- Card loading state --&gt;
{'{#if loading}'}
  &lt;SkeletonCard lines={'{4}'} /&gt;
{'{:else}'}
  &lt;Card&gt;...&lt;/Card&gt;
{'{/if}'}</pre>
	</section>
</div>
