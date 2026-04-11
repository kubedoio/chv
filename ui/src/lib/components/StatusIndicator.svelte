<script lang="ts">
  import { Activity, CheckCircle, AlertCircle, Clock, Loader2 } from 'lucide-svelte';

  interface Props {
    status: string;
    size?: 'sm' | 'md' | 'lg';
    showLabel?: boolean;
  }

  let { status, size = 'md', showLabel = true }: Props = $props();

  const sizeClasses = {
    sm: 'w-2 h-2',
    md: 'w-3 h-3',
    lg: 'w-4 h-4'
  };

  // Use separate derived values instead of an object to avoid reactivity issues
  let statusKey = $derived(status.toLowerCase());
  
  function getIconComponent(key: string) {
    switch (key) {
      case 'running':
      case 'ready':
      case 'active':
      case 'completed':
      case 'success':
        return CheckCircle;
      case 'starting':
      case 'stopping':
      case 'provisioning':
      case 'downloading':
      case 'importing':
      case 'pending':
        return Loader2;
      case 'stopped':
      case 'inactive':
        return Clock;
      case 'error':
      case 'failed':
        return AlertCircle;
      default:
        return Activity;
    }
  }
  
  function getColorClass(key: string): string {
    switch (key) {
      case 'running':
      case 'ready':
      case 'active':
      case 'completed':
      case 'success':
        return 'text-green-600';
      case 'starting':
      case 'stopping':
      case 'provisioning':
      case 'downloading':
      case 'importing':
      case 'pending':
        return 'text-blue-600';
      case 'stopped':
      case 'inactive':
        return 'text-gray-500';
      case 'error':
      case 'failed':
        return 'text-red-600';
      default:
        return 'text-gray-500';
    }
  }
  
  function getBgClass(key: string): string {
    switch (key) {
      case 'running':
      case 'ready':
      case 'active':
      case 'completed':
      case 'success':
        return 'bg-green-100';
      case 'starting':
      case 'stopping':
      case 'provisioning':
      case 'downloading':
      case 'importing':
      case 'pending':
        return 'bg-blue-100';
      case 'stopped':
      case 'inactive':
        return 'bg-gray-100';
      case 'error':
      case 'failed':
        return 'bg-red-100';
      default:
        return 'bg-gray-100';
    }
  }
  
  function shouldPulse(key: string): boolean {
    return ['starting', 'stopping', 'provisioning', 'downloading', 'importing', 'pending'].includes(key);
  }
  
  let IconComponent = $derived(getIconComponent(statusKey));
  let colorClass = $derived(getColorClass(statusKey));
  let bgClass = $derived(getBgClass(statusKey));
  let pulse = $derived(shouldPulse(statusKey));
</script>

<div class="flex items-center gap-2">
  <div class="relative">
    <div class="{sizeClasses[size]} rounded-full {bgClass} flex items-center justify-center">
      <IconComponent 
        class="{sizeClasses[size]} {colorClass} {pulse ? 'animate-spin' : ''}" 
      />
    </div>
    {#if pulse}
      <div class="absolute inset-0 rounded-full {bgClass} animate-ping opacity-75"></div>
    {/if}
  </div>
  {#if showLabel}
    <span class="text-sm font-medium capitalize {colorClass}">{status}</span>
  {/if}
</div>
