<script lang="ts">
  import { 
    Play, 
    Square, 
    RotateCcw, 
    Trash2, 
    Plus, 
    Camera,
    AlertCircle,
    CheckCircle2,
    Clock,
    MoreHorizontal
  } from 'lucide-svelte';

  interface TimelineEvent {
    id: string;
    timestamp: string;
    type: 'start' | 'stop' | 'restart' | 'create' | 'delete' | 'snapshot' | 'restore' | 'error' | 'info';
    title: string;
    description?: string;
    user?: string;
    status?: 'success' | 'failed' | 'pending';
  }

  interface Props {
    events: TimelineEvent[];
    maxItems?: number;
    showLoadMore?: boolean;
    onLoadMore?: () => void;
  }

  let { 
    events, 
    maxItems = 10, 
    showLoadMore = false,
    onLoadMore 
  }: Props = $props();

  let expandedItems = $state<Set<string>>(new Set());

  const displayedEvents = $derived(events.slice(0, maxItems));
  const hasMore = $derived(events.length > maxItems);

  const eventConfig = {
    start: { icon: Play, color: 'text-success', bg: 'bg-success/10', border: 'border-success/30' },
    stop: { icon: Square, color: 'text-warning', bg: 'bg-warning/10', border: 'border-warning/30' },
    restart: { icon: RotateCcw, color: 'text-primary', bg: 'bg-primary/10', border: 'border-primary/30' },
    create: { icon: Plus, color: 'text-info', bg: 'bg-info/10', border: 'border-info/30' },
    delete: { icon: Trash2, color: 'text-danger', bg: 'bg-danger/10', border: 'border-danger/30' },
    snapshot: { icon: Camera, color: 'text-info', bg: 'bg-info/10', border: 'border-info/30' },
    restore: { icon: RotateCcw, color: 'text-primary', bg: 'bg-primary/10', border: 'border-primary/30' },
    error: { icon: AlertCircle, color: 'text-danger', bg: 'bg-danger/10', border: 'border-danger/30' },
    info: { icon: MoreHorizontal, color: 'text-muted', bg: 'bg-chrome', border: 'border-line' }
  };

  function formatTime(timestamp: string): string {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);
    
    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    
    return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  function formatFullTime(timestamp: string): string {
    return new Date(timestamp).toLocaleString(undefined, {
      weekday: 'short',
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  }

  function toggleExpanded(id: string) {
    const newSet = new Set(expandedItems);
    if (newSet.has(id)) {
      newSet.delete(id);
    } else {
      newSet.add(id);
    }
    expandedItems = newSet;
  }

  function getStatusIcon(status?: string) {
    switch (status) {
      case 'success': return CheckCircle2;
      case 'failed': return AlertCircle;
      case 'pending': return Clock;
      default: return null;
    }
  }
</script>

<div class="timeline">
  {#if displayedEvents.length === 0}
    <div class="text-center py-8 text-muted">
      <Clock size={32} class="mx-auto mb-2 opacity-30" />
      <p class="text-sm">No activity recorded yet</p>
    </div>
  {:else}
    <div class="space-y-0">
      {#each displayedEvents as event, index}
        {@const config = eventConfig[event.type]}
        {@const StatusIcon = getStatusIcon(event.status)}
        {@const isExpanded = expandedItems.has(event.id)}
        {@const isLast = index === displayedEvents.length - 1}
        
        <div class="timeline-item group">
          <div class="flex gap-4">
            <!-- Timeline line and icon -->
            <div class="flex flex-col items-center">
              <!-- Icon -->
              <div class={`timeline-icon w-8 h-8 rounded-full flex items-center justify-center ${config.bg} ${config.border} border`}>
                <config.icon size={14} class={config.color} />
              </div>
              
              <!-- Vertical line -->
              {#if !isLast}
                <div class="timeline-line w-px flex-1 bg-line group-hover:bg-primary/30 transition-colors my-2"></div>
              {/if}
            </div>

            <!-- Content -->
            <div class={`flex-1 pb-6 ${!isLast ? '' : ''}`}>
              <div 
                class="timeline-content p-3 rounded-lg bg-white border border-line hover:border-primary/30 hover:shadow-sm transition-all"
                class:cursor-pointer={!!event.description}
                onclick={() => event.description && toggleExpanded(event.id)}
                onkeydown={(e) => e.key === 'Enter' && event.description && toggleExpanded(event.id)}
                role={event.description ? 'button' : undefined}
                tabindex={event.description ? 0 : undefined}
              >
                <div class="flex items-start justify-between gap-3">
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2 flex-wrap">
                      <span class="font-medium text-sm text-ink">{event.title}</span>
                      {#if StatusIcon}
                        <StatusIcon 
                          size={14} 
                          class={event.status === 'success' ? 'text-success' : event.status === 'failed' ? 'text-danger' : 'text-warning'} 
                        />
                      {/if}
                    </div>
                    
                    {#if event.user}
                      <p class="text-xs text-muted mt-0.5">by {event.user}</p>
                    {/if}
                  </div>
                  
                  <time 
                    class="text-xs text-muted whitespace-nowrap" 
                    title={formatFullTime(event.timestamp)}
                  >
                    {formatTime(event.timestamp)}
                  </time>
                </div>

                <!-- Expandable description -->
                {#if event.description}
                  <div 
                    class="overflow-hidden transition-all duration-200"
                    style="max-height: {isExpanded ? '200px' : '0'}; opacity: {isExpanded ? '1' : '0'};"
                  >
                    <p class="text-xs text-muted mt-2 pt-2 border-t border-line">
                      {event.description}
                    </p>
                  </div>
                  
                  {#if !isExpanded}
                    <div class="mt-1 text-xs text-primary">Click to expand</div>
                  {/if}
                {/if}
              </div>
            </div>
          </div>
        </div>
      {/each}
    </div>

    <!-- Load more -->
    {#if hasMore || showLoadMore}
      <button
        onclick={onLoadMore}
        class="w-full py-2 text-xs text-muted hover:text-primary hover:bg-chrome rounded transition-colors"
      >
        Load more events
      </button>
    {/if}
  {/if}
</div>

<style>
  .timeline {
    position: relative;
  }
  
  .timeline-item {
    position: relative;
  }
  
  .timeline-icon {
    transition: all 0.2s ease;
  }
  
  .timeline-item:hover .timeline-icon {
    transform: scale(1.1);
  }
  
  .timeline-line {
    transition: all 0.2s ease;
  }
</style>
