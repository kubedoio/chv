<script lang="ts">
  export let label: string;

  const toneClass = (value: string) => {
    switch (value) {
      case 'ready':
      case 'running':
      case 'active':
      case 'succeeded':
        return 'border-success text-success';
      case 'degraded':
      case 'warning':
      case 'starting':
      case 'stopping':
      case 'importing':
        return 'border-warning text-warning';
      case 'error':
      case 'failed':
      case 'missing_prerequisites':
      case 'drift_detected':
        return 'border-danger text-danger';
      default:
        return 'border-line text-muted';
    }
  };
</script>

<span class={`inline-flex items-center gap-2 border px-2 py-1 text-[12px] font-medium uppercase tracking-[0.12em] ${toneClass(label)}`}>
  <span class="text-[10px]">●</span>
  {label.replaceAll('_', ' ')}
</span>

