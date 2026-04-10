<script lang="ts">
  import { TrendingUp, TrendingDown, Minus, Loader2 } from 'lucide-svelte';

  interface Props {
    title: string;
    value: string | number;
    icon?: typeof TrendingUp;
    iconColor?: 'blue' | 'green' | 'amber' | 'purple' | 'red' | 'slate';
    trend?: 'up' | 'down' | 'neutral';
    trendValue?: string;
    subtitle?: string;
    loading?: boolean;
    progress?: {
      value: number;
      max: number;
      label?: string;
    };
    sparklineData?: number[];
    href?: string;
  }

  let {
    title,
    value,
    icon: Icon,
    iconColor = 'slate',
    trend,
    trendValue,
    subtitle,
    loading = false,
    progress,
    sparklineData,
    href
  }: Props = $props();

  const iconColorClasses = {
    blue: 'bg-blue-50 text-blue-600',
    green: 'bg-green-50 text-green-600',
    amber: 'bg-amber-50 text-amber-600',
    purple: 'bg-purple-50 text-purple-600',
    red: 'bg-red-50 text-red-600',
    slate: 'bg-slate-100 text-slate-600'
  };

  const trendConfig = {
    up: { icon: TrendingUp, colorClass: 'text-green-600', bgClass: 'bg-green-50' },
    down: { icon: TrendingDown, colorClass: 'text-red-600', bgClass: 'bg-red-50' },
    neutral: { icon: Minus, colorClass: 'text-slate-500', bgClass: 'bg-slate-100' }
  };

  const percentage = $derived(
    progress ? Math.min(100, Math.max(0, (progress.value / progress.max) * 100)) : 0
  );

  // Calculate sparkline points
  const sparklinePoints = $derived(() => {
    if (!sparklineData || sparklineData.length < 2) return [];
    const min = Math.min(...sparklineData);
    const max = Math.max(...sparklineData);
    const range = max - min || 1;
    const width = 60;
    const height = 24;
    
    return sparklineData.map((point, i) => ({
      x: (i / (sparklineData.length - 1)) * width,
      y: height - ((point - min) / range) * 20 - 2,
      hasPrev: i > 0
    }));
  });
</script>

{#if href}
  <a
    {href}
    class="resource-card card card-interactive block no-underline text-inherit"
    aria-label={title}
  >
    <div class="p-5">
      {#if loading}
        <div class="animate-pulse">
          <div class="flex items-start justify-between">
            <div class="flex-1">
              <div class="h-3 w-20 bg-slate-200 rounded mb-3"></div>
              <div class="h-8 w-16 bg-slate-200 rounded"></div>
            </div>
            <div class="h-10 w-10 bg-slate-200 rounded-lg"></div>
          </div>
          <div class="mt-4 pt-4 border-t border-slate-100">
            <div class="h-2 w-full bg-slate-200 rounded-full"></div>
          </div>
        </div>
      {:else}
        <div class="flex items-start justify-between">
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-xs font-medium text-slate-500 uppercase tracking-wider">
                {title}
              </span>
              {#if trend && trendValue}
                {@const config = trendConfig[trend]}
                <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium {config.bgClass} {config.colorClass}">
                  <config.icon size={10} />
                  {trendValue}
                </span>
              {/if}
            </div>
            <div class="flex items-center gap-3 mt-1">
              <span class="text-2xl font-bold text-slate-900">{value}</span>
              {#if sparklineData && sparklineData.length > 1}
                <svg width="60" height="24" viewBox="0 0 60 24" class="sparkline-mini">
                  {#each sparklinePoints() as point, i}
                    <circle cx={point.x} cy={point.y} r="1.5" fill="var(--color-primary)" />
                    {#if point.hasPrev}
                      {@const prev = sparklinePoints()[i - 1]}
                      <line 
                        x1={prev.x} y1={prev.y} 
                        x2={point.x} y2={point.y} 
                        stroke="var(--color-primary)" 
                        stroke-width="1.5"
                        stroke-linecap="round"
                      />
                    {/if}
                  {/each}
                </svg>
              {/if}
            </div>
            {#if subtitle}
              <p class="text-xs text-slate-500 mt-1">{subtitle}</p>
            {/if}
          </div>
          {#if Icon}
            <div class="p-2.5 rounded-lg {iconColorClasses[iconColor]} transition-colors">
              <Icon size={20} />
            </div>
          {/if}
        </div>

        {#if progress}
          <div class="mt-4 pt-4 border-t border-slate-100">
            <div class="flex items-center justify-between text-sm mb-1.5">
              <span class="text-slate-500">{progress.label || 'Usage'}</span>
              <span class="font-medium text-slate-700">{percentage.toFixed(0)}%</span>
            </div>
            <div class="w-full bg-slate-100 rounded-full h-2 overflow-hidden">
              <div
                class="h-2 rounded-full transition-all duration-500 ease-out progress-bar"
                style="width: {percentage}%"
              ></div>
            </div>
            <p class="text-xs text-slate-500 mt-1.5">
              {progress.value.toLocaleString()} of {progress.max.toLocaleString()}
            </p>
          </div>
        {/if}
      {/if}
    </div>
  </a>
{:else}
  <div class="resource-card card" role="region" aria-label={title}>
    <div class="p-5">
      {#if loading}
        <div class="animate-pulse">
          <div class="flex items-start justify-between">
            <div class="flex-1">
              <div class="h-3 w-20 bg-slate-200 rounded mb-3"></div>
              <div class="h-8 w-16 bg-slate-200 rounded"></div>
            </div>
            <div class="h-10 w-10 bg-slate-200 rounded-lg"></div>
          </div>
          <div class="mt-4 pt-4 border-t border-slate-100">
            <div class="h-2 w-full bg-slate-200 rounded-full"></div>
          </div>
        </div>
      {:else}
        <div class="flex items-start justify-between">
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-xs font-medium text-slate-500 uppercase tracking-wider">
                {title}
              </span>
              {#if trend && trendValue}
                {@const config = trendConfig[trend]}
                <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium {config.bgClass} {config.colorClass}">
                  <config.icon size={10} />
                  {trendValue}
                </span>
              {/if}
            </div>
            <div class="flex items-center gap-3 mt-1">
              <span class="text-2xl font-bold text-slate-900">{value}</span>
              {#if sparklineData && sparklineData.length > 1}
                <svg width="60" height="24" viewBox="0 0 60 24" class="sparkline-mini">
                  {#each sparklinePoints() as point, i}
                    <circle cx={point.x} cy={point.y} r="1.5" fill="var(--color-primary)" />
                    {#if point.hasPrev}
                      {@const prev = sparklinePoints()[i - 1]}
                      <line 
                        x1={prev.x} y1={prev.y} 
                        x2={point.x} y2={point.y} 
                        stroke="var(--color-primary)" 
                        stroke-width="1.5"
                        stroke-linecap="round"
                      />
                    {/if}
                  {/each}
                </svg>
              {/if}
            </div>
            {#if subtitle}
              <p class="text-xs text-slate-500 mt-1">{subtitle}</p>
            {/if}
          </div>
          {#if Icon}
            <div class="p-2.5 rounded-lg {iconColorClasses[iconColor]} transition-colors">
              <Icon size={20} />
            </div>
          {/if}
        </div>

        {#if progress}
          <div class="mt-4 pt-4 border-t border-slate-100">
            <div class="flex items-center justify-between text-sm mb-1.5">
              <span class="text-slate-500">{progress.label || 'Usage'}</span>
              <span class="font-medium text-slate-700">{percentage.toFixed(0)}%</span>
            </div>
            <div class="w-full bg-slate-100 rounded-full h-2 overflow-hidden">
              <div
                class="h-2 rounded-full transition-all duration-500 ease-out progress-bar"
                style="width: {percentage}%"
              ></div>
            </div>
            <p class="text-xs text-slate-500 mt-1.5">
              {progress.value.toLocaleString()} of {progress.max.toLocaleString()}
            </p>
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .resource-card {
    transition: 
      box-shadow var(--duration-normal) var(--ease-default),
      border-color var(--duration-normal) var(--ease-default),
      transform var(--duration-normal) var(--ease-default);
  }

  .resource-card:hover {
    box-shadow: var(--shadow-md);
  }

  .card-interactive:hover {
    box-shadow: var(--shadow-lg);
    border-color: rgba(229, 112, 53, 0.3);
    transform: translateY(-2px);
  }

  .progress-bar {
    background: linear-gradient(90deg, var(--color-primary) 0%, var(--color-primary-hover) 100%);
  }

  .sparkline-mini {
    opacity: 0.7;
  }

  .resource-card:hover .sparkline-mini {
    opacity: 1;
  }
</style>
