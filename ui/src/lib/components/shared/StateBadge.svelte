<script lang="ts">
  interface Props {
    label: string;
  }

  let { label }: Props = $props();

  const tone = $derived.by(() => {
    const val = label.toLowerCase();
    switch (val) {
      case 'ready':
      case 'running':
      case 'active':
      case 'succeeded':
        return 'bg-success/15 text-success-dark border-success/20 glow-success';
      case 'degraded':
      case 'warning':
      case 'starting':
      case 'stopping':
      case 'importing':
      case 'provisioning':
      case 'prepared':
        return 'bg-warning/15 text-warning-dark border-warning/20 glow-warning';
      case 'error':
      case 'failed':
      case 'missing_prerequisites':
      case 'drift_detected':
        return 'bg-danger/15 text-danger-dark border-danger/20 glow-danger';
      default:
        return 'bg-slate-100 text-slate-600 border-slate-200';
    }
  });

  const transitioning = $derived(['starting', 'stopping', 'importing', 'provisioning'].includes(label.toLowerCase()));
</script>

<span class={`state-badge inline-flex items-center gap-1.5 border px-2.5 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider ${tone}`}>
  <span class={`status-dot w-1.5 h-1.5 rounded-full ${transitioning ? 'animate-pulse' : ''}`} style="background-color: currentColor"></span>
  {label.replaceAll('_', ' ')}
</span>

<style>
  .state-badge {
    transition: all 0.3s ease;
  }
  
  .text-success-dark { color: #15803d; }
  .text-warning-dark { color: #a16207; }
  .text-danger-dark { color: #b91c1c; }
  
  .glow-success { box-shadow: 0 0 10px rgba(34, 197, 94, 0.1); }
  .glow-warning { box-shadow: 0 0 10px rgba(234, 179, 8, 0.1); }
  .glow-danger { box-shadow: 0 0 10px rgba(239, 68, 68, 0.1); }

  .bg-success\/15 { background-color: rgba(34, 197, 94, 0.15); }
  .bg-warning\/15 { background-color: rgba(234, 179, 8, 0.15); }
  .bg-danger\/15 { background-color: rgba(239, 68, 68, 0.15); }

  .border-success\/20 { border-color: rgba(34, 197, 94, 0.2); }
  .border-warning\/20 { border-color: rgba(234, 179, 8, 0.2); }
  .border-danger\/20 { border-color: rgba(239, 68, 68, 0.2); }
</style>

