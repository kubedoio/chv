<script lang="ts">
  import { onMount } from 'svelte';
  import { Download, ZoomIn, ZoomOut, Maximize2 } from 'lucide-svelte';
  
  interface DataPoint {
    timestamp: string;
    value: number;
  }
  
  interface Series {
    label: string;
    data: DataPoint[];
    color: string;
    unit?: string;
  }
  
  interface Props {
    title: string;
    series: Series[];
    timeRange?: '1h' | '6h' | '24h' | '7d';
    onTimeRangeChange?: (range: '1h' | '6h' | '24h' | '7d') => void;
    onExport?: () => void;
    height?: number;
    showLegend?: boolean;
  }
  
  let { 
    title, 
    series, 
    timeRange = '1h',
    onTimeRangeChange,
    onExport,
    height = 200,
    showLegend = true
  }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let container = $state<HTMLDivElement | null>(null);
  let isFullscreen = $state(false);
  let zoomLevel = $state(1);
  let panOffset = $state(0);
  let isDragging = $state(false);
  let dragStartX = $state(0);
  let dragStartPan = $state(0);
  let hoveredPoint = $state<{ seriesIndex: number; pointIndex: number; x: number; y: number } | null>(null);
  let mousePos = $state<{ x: number; y: number } | null>(null);

  const timeRanges = [
    { label: '1H', value: '1h' },
    { label: '6H', value: '6h' },
    { label: '24H', value: '24h' },
    { label: '7D', value: '7d' }
  ] as const;

  $effect(() => {
    if (canvas && series.length > 0) {
      drawChart();
    }
  });

  function drawChart() {
    if (!canvas) return;
    
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const width = rect.width;
    const chartHeight = rect.height - 30; // Leave room for legend
    
    ctx.clearRect(0, 0, width, rect.height);

    // Draw grid
    drawGrid(ctx, width, chartHeight);

    // Draw each series
    series.forEach((s, index) => {
      drawSeries(ctx, s, width, chartHeight, index);
    });

    // Draw hover line and tooltip
    if (mousePos && !isDragging) {
      drawHoverIndicator(ctx, width, chartHeight);
    }
  }

  function drawGrid(ctx: CanvasRenderingContext2D, width: number, height: number) {
    ctx.strokeStyle = '#f1f5f9';
    ctx.lineWidth = 1;

    // Horizontal grid lines
    for (let i = 0; i <= 4; i++) {
      const y = (height / 4) * i;
      ctx.beginPath();
      ctx.moveTo(40, y);
      ctx.lineTo(width - 10, y);
      ctx.stroke();
    }
  }

  function drawSeries(ctx: CanvasRenderingContext2D, series: Series, width: number, height: number, index: number) {
    if (series.data.length < 2) return;

    const padding = 40;
    const chartWidth = width - padding - 10;
    
    // Find min/max for scaling
    const allValues = series.data.map(d => d.value);
    const maxValue = Math.max(...allValues) * 1.1 || 1;
    const minValue = Math.min(...allValues) * 0.9;
    const valueRange = maxValue - minValue || 1;

    // Draw area fill
    const gradient = ctx.createLinearGradient(0, 0, 0, height);
    gradient.addColorStop(0, `${series.color}40`); // 25% opacity
    gradient.addColorStop(1, `${series.color}00`); // 0% opacity

    ctx.beginPath();
    ctx.fillStyle = gradient;
    
    series.data.forEach((point, i) => {
      const x = padding + (i / (series.data.length - 1)) * chartWidth;
      const y = height - ((point.value - minValue) / valueRange) * height;
      
      if (i === 0) {
        ctx.moveTo(x, height);
        ctx.lineTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
      
      if (i === series.data.length - 1) {
        ctx.lineTo(x, height);
      }
    });
    
    ctx.closePath();
    ctx.fill();

    // Draw line
    ctx.beginPath();
    ctx.strokeStyle = series.color;
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';

    series.data.forEach((point, i) => {
      const x = padding + (i / (series.data.length - 1)) * chartWidth;
      const y = height - ((point.value - minValue) / valueRange) * height;
      
      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });
    
    ctx.stroke();

    // Draw points on hover
    if (hoveredPoint && hoveredPoint.seriesIndex === index) {
      const point = series.data[hoveredPoint.pointIndex];
      const x = padding + (hoveredPoint.pointIndex / (series.data.length - 1)) * chartWidth;
      const y = height - ((point.value - minValue) / valueRange) * height;
      
      ctx.beginPath();
      ctx.fillStyle = series.color;
      ctx.arc(x, y, 4, 0, Math.PI * 2);
      ctx.fill();
      
      ctx.beginPath();
      ctx.strokeStyle = 'white';
      ctx.lineWidth = 2;
      ctx.arc(x, y, 6, 0, Math.PI * 2);
      ctx.stroke();
    }
  }

  function drawHoverIndicator(ctx: CanvasRenderingContext2D, width: number, height: number) {
    if (!mousePos) return;
    
    const x = mousePos.x;
    
    // Draw vertical line
    ctx.beginPath();
    ctx.strokeStyle = 'rgba(148, 163, 184, 0.5)';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 4]);
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  function handleMouseMove(e: MouseEvent) {
    if (!canvas) return;
    
    const rect = canvas.getBoundingClientRect();
    mousePos = {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top
    };

    if (isDragging) {
      const deltaX = e.clientX - dragStartX;
      panOffset = Math.max(0, Math.min(dragStartPan - deltaX, 100));
    } else {
      // Find nearest point
      const padding = 40;
      const chartWidth = rect.width - padding - 10;
      const relativeX = mousePos.x - padding;
      const dataIndex = Math.round((relativeX / chartWidth) * (series[0]?.data.length - 1 || 0));
      
      if (dataIndex >= 0 && dataIndex < (series[0]?.data.length || 0)) {
        hoveredPoint = {
          seriesIndex: 0,
          pointIndex: dataIndex,
          x: mousePos.x,
          y: mousePos.y
        };
      }
    }

    drawChart();
  }

  function handleMouseLeave() {
    mousePos = null;
    hoveredPoint = null;
    isDragging = false;
    drawChart();
  }

  function handleMouseDown(e: MouseEvent) {
    isDragging = true;
    dragStartX = e.clientX;
    dragStartPan = panOffset;
  }

  function handleMouseUp() {
    isDragging = false;
  }

  function toggleFullscreen() {
    isFullscreen = !isFullscreen;
    setTimeout(() => drawChart(), 100);
  }

  function zoomIn() {
    zoomLevel = Math.min(zoomLevel * 1.2, 5);
    drawChart();
  }

  function zoomOut() {
    zoomLevel = Math.max(zoomLevel / 1.2, 0.5);
    drawChart();
  }

  function formatValue(value: number, unit?: string): string {
    if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M${unit || ''}`;
    if (value >= 1000) return `${(value / 1000).toFixed(1)}K${unit || ''}`;
    return `${value.toFixed(1)}${unit || ''}`;
  }
</script>

<div 
  class={`metrics-chart bg-white border border-line rounded-lg overflow-hidden ${isFullscreen ? 'fixed inset-4 z-50' : ''}`}
  bind:this={container}
>
  <!-- Header -->
  <div class="flex items-center justify-between px-4 py-3 border-b border-line bg-chrome/50">
    <h3 class="text-sm font-semibold text-ink">{title}</h3>
    
    <div class="flex items-center gap-2">
      <!-- Time range selector -->
      <div class="flex items-center bg-white rounded border border-line overflow-hidden">
        {#each timeRanges as range}
          <button
            onclick={() => onTimeRangeChange?.(range.value)}
            class={`px-2 py-1 text-xs font-medium transition-colors ${
              timeRange === range.value 
                ? 'bg-primary text-white' 
                : 'text-muted hover:text-ink hover:bg-chrome'
            }`}
          >
            {range.label}
          </button>
        {/each}
      </div>

      <!-- Zoom controls -->
      <div class="flex items-center gap-1">
        <button
          onclick={zoomOut}
          class="p-1.5 text-muted hover:text-ink hover:bg-chrome rounded transition-colors"
          title="Zoom out"
        >
          <ZoomOut size={14} />
        </button>
        <button
          onclick={zoomIn}
          class="p-1.5 text-muted hover:text-ink hover:bg-chrome rounded transition-colors"
          title="Zoom in"
        >
          <ZoomIn size={14} />
        </button>
      </div>

      <!-- Export button -->
      {#if onExport}
        <button
          onclick={onExport}
          class="p-1.5 text-muted hover:text-primary hover:bg-primary/10 rounded transition-colors"
          title="Export to CSV"
        >
          <Download size={14} />
        </button>
      {/if}

      <!-- Fullscreen toggle -->
      <button
        onclick={toggleFullscreen}
        class="p-1.5 text-muted hover:text-ink hover:bg-chrome rounded transition-colors"
        title={isFullscreen ? 'Exit fullscreen' : 'Fullscreen'}
      >
        <Maximize2 size={14} />
      </button>
    </div>
  </div>

  <!-- Chart canvas -->
  <div class="relative">
    <canvas
      bind:this={canvas}
      class="w-full cursor-crosshair"
      style="height: {isFullscreen ? 'calc(100vh - 200px)' : height + 'px'};"
      onmousemove={handleMouseMove}
      onmouseleave={handleMouseLeave}
      onmousedown={handleMouseDown}
      onmouseup={handleMouseUp}
    ></canvas>

    <!-- Tooltip -->
    {#if hoveredPoint && series[hoveredPoint.seriesIndex]}
      {@const s = series[hoveredPoint.seriesIndex]}
      {@const point = s.data[hoveredPoint.pointIndex]}
      <div 
        class="absolute pointer-events-none bg-ink text-white text-xs rounded px-2 py-1 shadow-lg z-10"
        style="left: {Math.min(hoveredPoint.x + 10, (container?.offsetWidth || 300) - 100)}px; top: {Math.max(hoveredPoint.y - 40, 10)}px;"
      >
        <div class="font-medium">{s.label}</div>
        <div>{formatValue(point.value, s.unit)}</div>
        <div class="text-white/70 text-[10px]">{new Date(point.timestamp).toLocaleTimeString()}</div>
      </div>
    {/if}
  </div>

  <!-- Legend -->
  {#if showLegend && series.length > 0}
    <div class="flex items-center justify-center gap-4 px-4 py-2 border-t border-line bg-chrome/30">
      {#each series as s}
        <div class="flex items-center gap-1.5">
          <div class="w-3 h-3 rounded-full" style="background-color: {s.color}"></div>
          <span class="text-xs text-muted">{s.label}</span>
          {#if s.data.length > 0}
            {@const lastValue = s.data[s.data.length - 1].value}
            <span class="text-xs font-medium text-ink">{formatValue(lastValue, s.unit)}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
