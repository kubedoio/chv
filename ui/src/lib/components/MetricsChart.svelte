<script lang="ts">
  import { onMount, $effect } from 'svelte';
  
  interface DataPoint {
    label: string;
    value: number;
    max: number;
    color: string;
    history?: number[];
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

  let canvases = $state<Record<string, HTMLCanvasElement>>({});

  $effect(() => {
    data.forEach(item => {
      if (item.history && canvases[item.label]) {
        drawSparkline(canvases[item.label], item.history, item.color, item.max);
      }
    });
  });

  function drawSparkline(canvas: HTMLCanvasElement, history: number[], color: string, max: number) {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    ctx.clearRect(0, 0, width, height);

    // Draw subtle grid lines
    ctx.beginPath();
    ctx.strokeStyle = '#f1f5f9';
    ctx.lineWidth = 1;
    [0.25, 0.5, 0.75].forEach(pct => {
      const y = height - (pct * height);
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
    });
    ctx.stroke();

    if (history.length < 2) return;

    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = 2.5;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';

    const step = width / (60 - 1); // Assume 60 points max
    const points = history.slice(-60);
    const startX = width - (points.length - 1) * step;

    points.forEach((val, i) => {
      const x = startX + i * step;
      const y = height - (val / max) * height;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });

    ctx.stroke();

    // Fill area with premium gradient
    const gradient = ctx.createLinearGradient(0, 0, 0, height);
    gradient.addColorStop(0, `${color}33`); // 20% opacity
    gradient.addColorStop(1, `${color}00`); // 0% opacity
    
    ctx.lineTo(width, height);
    ctx.lineTo(startX, height);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();
  }
</script>

<div class="metrics-chart bg-white border border-line rounded p-4">
  <h3 class="text-sm font-semibold text-ink mb-4 tracking-tight">{title}</h3>
  
  <div class="space-y-6">
    {#each data as item}
      <div class="metric-item">
        <div class="flex justify-between items-end mb-2">
          <div>
            <span class="text-[11px] uppercase tracking-wider text-muted font-bold">{item.label}</span>
            <div class="text-xl font-mono font-bold" style="color: {item.color}">
              {item.value.toFixed(1)}{item.label.includes('Usage') ? '%' : ' MB'}
            </div>
          </div>
          {#if item.history && item.history.length > 0}
            <canvas 
              bind:this={canvases[item.label]} 
              width="120" 
              height="40" 
              class="h-10 w-32"
            ></canvas>
          {/if}
        </div>
        <div class="w-full bg-chrome rounded-full h-1.5 overflow-hidden">
          <div 
            class="h-full transition-all duration-700 ease-out"
            style="width: {Math.min(100, (item.value / item.max) * 100)}%; background-color: {item.color}"
          ></div>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .metrics-chart {
    min-width: 280px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.05);
  }
</style>
