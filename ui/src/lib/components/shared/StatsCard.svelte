<script lang="ts">
	import type { Component } from 'svelte';
	import { TrendingUp, TrendingDown, Minus, ChevronRight } from 'lucide-svelte';

	interface Props {
		title: string;
		value: string | number;
		icon?: Component;
		trend?: 'up' | 'down' | 'neutral';
		subtitle?: string;
		href?: string;
	}

	let { title, value, icon: Icon, trend, subtitle, href }: Props = $props();

	const trendConfig = {
		up: { icon: TrendingUp, colorClass: 'text-success' },
		down: { icon: TrendingDown, colorClass: 'text-danger' },
		neutral: { icon: Minus, colorClass: 'text-light' }
	};
</script>

{#if href}
	<a 
		{href}
		class="rounded border border-line bg-chrome p-4 block no-underline text-inherit hover:shadow-md transition-shadow"
		aria-label={title}
	>
		<div class="flex items-start justify-between">
			<div class="flex-1">
				<div class="mb-2 text-[11px] uppercase tracking-[0.08em] text-muted">
					{title}
				</div>
				<div class="flex items-center gap-2">
					{#if Icon}
						<Icon size={24} class="text-muted" aria-hidden="true" />
					{/if}
					<span class="text-[32px] font-semibold text-ink">
						{value}
					</span>
				</div>
				{#if subtitle}
					<div class="mt-1 text-sm text-muted">
						{subtitle}
					</div>
				{/if}
			</div>
			{#if trend}
				{@const TrendIcon = trendConfig[trend].icon}
				{@const trendColor = trendConfig[trend].colorClass}
				<div class="flex-shrink-0">
					<TrendIcon size={20} class={trendColor} aria-hidden="true" />
				</div>
			{:else}
				<div class="flex-shrink-0">
					<ChevronRight size={20} class="text-muted" aria-hidden="true" />
				</div>
			{/if}
		</div>
	</a>
{:else}
	<div 
		class="rounded border border-line bg-chrome p-4"
		role="region"
		aria-label={title}
	>
		<div class="flex items-start justify-between">
			<div class="flex-1">
				<div class="mb-2 text-[11px] uppercase tracking-[0.08em] text-muted">
					{title}
				</div>
				<div class="flex items-center gap-2">
					{#if Icon}
						<Icon size={24} class="text-muted" aria-hidden="true" />
					{/if}
					<span class="text-[32px] font-semibold text-ink">
						{value}
					</span>
				</div>
				{#if subtitle}
					<div class="mt-1 text-sm text-muted">
						{subtitle}
					</div>
				{/if}
			</div>
			{#if trend}
				{@const TrendIcon = trendConfig[trend].icon}
				{@const trendColor = trendConfig[trend].colorClass}
				<div class="flex-shrink-0">
					<TrendIcon size={20} class={trendColor} aria-hidden="true" />
				</div>
			{/if}
		</div>
	</div>
{/if}
