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

  const config = $derived.by(() => {
    switch (status.toLowerCase()) {
      case 'running':
      case 'ready':
      case 'active':
      case 'completed':
      case 'success':
        return {
          icon: CheckCircle,
          color: 'text-green-600',
          bg: 'bg-green-100',
          pulse: false,
          label: status
        };
      case 'starting':
      case 'stopping':
      case 'provisioning':
      case 'downloading':
      case 'importing':
      case 'pending':
        return {
          icon: Loader2,
          color: 'text-blue-600',
          bg: 'bg-blue-100',
          pulse: true,
          label: status
        };
      case 'stopped':
      case 'inactive':
        return {
          icon: Clock,
          color: 'text-gray-500',
          bg: 'bg-gray-100',
          pulse: false,
          label: status
        };
      case 'error':
      case 'failed':
        return {
          icon: AlertCircle,
          color: 'text-red-600',
          bg: 'bg-red-100',
          pulse: false,
          label: status
        };
      default:
        return {
          icon: Activity,
          color: 'text-gray-500',
          bg: 'bg-gray-100',
          pulse: false,
          label: status
        };
    }
  });
</script>

<div class="flex items-center gap-2">
  <div class="relative">
    <div class="{sizeClasses[size]} rounded-full {config.bg} flex items-center justify-center">
      <config.icon 
        class="{sizeClasses[size]} {config.color} {config.pulse ? 'animate-spin' : ''}" 
      />
    </div>
    {#if config.pulse}
      <div class="absolute inset-0 rounded-full {config.bg} animate-ping opacity-75"></div>
    {/if}
  </div>
  {#if showLabel}
    <span class="text-sm font-medium capitalize {config.color}">{config.label}</span>
  {/if}
</div>
