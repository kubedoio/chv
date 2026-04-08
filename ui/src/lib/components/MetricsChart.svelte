<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  
  interface DataPoint {
    label: string;
    value: number;
    max: number;
    color: string;
  }
  
  interface Props {
    title: string;
    data: DataPoint[];
    size?: 'sm' | 'md' | 'lg';
  }
  
  let { title, data, size = 'md' }: Props = $props();
  
  const sizeClasses = {
    sm: 'h-32',
    md: 'h-48',
    lg: 'h-64'
  };
</script>

<div class="metrics-chart bg-white border border-line rounded p-4">
  <h3 class="text-sm font-semibold text-gray-700 mb-3">{title}</h3>
  
  <div class="space-y-3">
    {#each data as item}
      <div class="metric-item">
        <div class="flex justify-between items-center mb-1">
          <span class="text-xs text-gray-600">{item.label}</span>
          <span class="text-xs font-medium" style="color: {item.color}">
            {item.value.toFixed(1)}%
          </span>
        </div>
        <div class="w-full bg-gray-200 rounded-full h-2">
          <div 
            class="h-2 rounded-full transition-all duration-500"
            style="width: {Math.min(100, (item.value / item.max) * 100)}%; background-color: {item.color}"
          ></div>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .metrics-chart {
    min-width: 200px;
  }
</style>
