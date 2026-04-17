<script lang="ts">
  interface Props {
    value: number;      // 0-100
    max?: number;       // Default 100
    label?: string;     // Optional label text
    showValue?: boolean; // Show percentage text
    size?: 'sm' | 'md' | 'lg';
    color?: 'blue' | 'green' | 'yellow' | 'red';
  }

  let { 
    value, 
    max = 100, 
    label = '', 
    showValue = true,
    size = 'md',
    color = 'blue'
  }: Props = $props();

  const percentage = $derived(Math.min(100, Math.max(0, (value / max) * 100)));

  const sizeClasses = {
    sm: 'h-1.5',
    md: 'h-2',
    lg: 'h-4'
  };

  const colorClasses = {
    blue: 'bg-blue-600',
    green: 'bg-green-600',
    yellow: 'bg-yellow-500',
    red: 'bg-red-600'
  };
</script>

<div class="w-full">
  {#if label}
    <div class="flex justify-between items-center mb-1">
      <span class="text-sm text-gray-600">{label}</span>
      {#if showValue}
        <span class="text-sm font-medium text-gray-900">{percentage.toFixed(0)}%</span>
      {/if}
    </div>
  {/if}
  
  <div class="w-full bg-gray-200 rounded-full {sizeClasses[size]}">
    <div 
      class="{colorClasses[color]} rounded-full transition-all duration-300 ease-out {sizeClasses[size]}"
      style="width: {percentage}%"
      role="progressbar"
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={max}
    ></div>
  </div>
</div>
