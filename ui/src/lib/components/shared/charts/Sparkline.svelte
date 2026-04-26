<script lang="ts">
  interface Props {
    data: number[];
    color?: string;
    width?: number;
    height?: number;
    showFill?: boolean;
    strokeWidth?: number;
  }

  let {
    data,
    color = 'var(--color-primary)',
    width = 120,
    height = 40,
    showFill = true,
    strokeWidth = 2
  }: Props = $props();

  const padding = 2;
  
  // Helper functions instead of derived objects to avoid re-renders
  function getMin() { return Math.min(...data, 0); }
  function getMax() { return Math.max(...data, 1); }
  function getRange() { return getMax() - getMin(); }
  function getChartMin() { return getMin(); }
  function getChartMax() { return getMax(); }
  function getChartRange() { return getRange(); }

  function getX(index: number): number {
    return padding + (index / (data.length - 1)) * (width - 2 * padding);
  }

  function getY(value: number): number {
    const range = getChartRange();
    const min = getChartMin();
    const normalized = range === 0 
      ? 0.5 
      : (value - min) / range;
    return height - padding - (normalized * (height - 2 * padding));
  }

  const pathD = $derived(() => {
    if (data.length === 0) return '';
    if (data.length === 1) {
      const x = getX(0);
      const y = getY(data[0]);
      return `M ${x} ${y}`;
    }

    let path = `M ${getX(0)} ${getY(data[0])}`;
    
    for (let i = 1; i < data.length; i++) {
      const x = getX(i);
      const y = getY(data[i]);
      const prevX = getX(i - 1);
      const prevY = getY(data[i - 1]);
      
      // Smooth curve using cubic bezier
      const cp1x = prevX + (x - prevX) / 3;
      const cp1y = prevY;
      const cp2x = prevX + 2 * (x - prevX) / 3;
      const cp2y = y;
      
      path += ` C ${cp1x} ${cp1y}, ${cp2x} ${cp2y}, ${x} ${y}`;
    }
    
    return path;
  });

  const fillPathD = $derived(() => {
    if (data.length === 0) return '';
    const linePath = pathD();
    const lastX = getX(data.length - 1);
    const firstX = getX(0);
    const bottomY = height - padding;
    
    return `${linePath} L ${lastX} ${bottomY} L ${firstX} ${bottomY} Z`;
  });
</script>

<svg 
  {width} 
  {height} 
  viewBox="0 0 {width} {height}"
  class="sparkline"
  aria-label="Trend chart"
  role="img"
>
  <defs>
    <linearGradient id="sparkline-gradient-{data.length}" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" style="stop-color: {color}; stop-opacity: 0.3" />
      <stop offset="100%" style="stop-color: {color}; stop-opacity: 0.05" />
    </linearGradient>
  </defs>
  
  {#if showFill && data.length > 1}
    <path
      d={fillPathD()}
      fill="url(#sparkline-gradient-{data.length})"
      class="sparkline-fill"
    />
  {/if}
  
  <path
    d={pathD()}
    fill="none"
    stroke={color}
    stroke-width={strokeWidth}
    stroke-linecap="round"
    stroke-linejoin="round"
    class="sparkline-stroke"
  />
</svg>

<style>
  .sparkline {
    overflow: visible;
  }
  
  .sparkline-stroke {
    transition: d 0.3s ease;
  }
  
  .sparkline-fill {
    transition: d 0.3s ease;
  }
</style>
