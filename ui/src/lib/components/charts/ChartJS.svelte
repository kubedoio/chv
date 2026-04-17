<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { Chart, ChartData, ChartOptions } from 'chart.js';
  import {
    Chart as ChartJS,
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    BarElement,
    Title,
    Tooltip,
    Legend,
    Filler,
    LineController,
    BarController
  } from 'chart.js';

  // Register Chart.js components
  ChartJS.register(
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    BarElement,
    Title,
    Tooltip,
    Legend,
    Filler,
    LineController,
    BarController
  );

  interface Props {
    type: 'line' | 'bar';
    data: ChartData;
    options?: ChartOptions;
    height?: number;
  }

  let { type, data, options = {}, height = 300 }: Props = $props();

  let canvas: HTMLCanvasElement;
  let chart: Chart | null = null;

  onMount(() => {
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    chart = new ChartJS(ctx, {
      type,
      data,
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: {
            display: true,
            position: 'top',
            labels: {
              usePointStyle: true,
              padding: 15,
              font: {
                size: 12,
                family: 'system-ui, -apple-system, sans-serif'
              }
            }
          },
          tooltip: {
            mode: 'index',
            intersect: false,
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            titleFont: {
              size: 13,
              family: 'system-ui, -apple-system, sans-serif'
            },
            bodyFont: {
              size: 12,
              family: 'system-ui, -apple-system, sans-serif'
            },
            padding: 12,
            cornerRadius: 8,
            displayColors: true
          }
        },
        scales: {
          x: {
            grid: {
              display: false
            },
            ticks: {
              font: {
                size: 11,
                family: 'system-ui, -apple-system, sans-serif'
              },
              color: '#64748b'
            }
          },
          y: {
            grid: {
              color: '#f1f5f9',
              drawBorder: false
            },
            ticks: {
              font: {
                size: 11,
                family: 'system-ui, -apple-system, sans-serif'
              },
              color: '#64748b'
            }
          }
        },
        interaction: {
          mode: 'nearest',
          axis: 'x',
          intersect: false
        },
        ...options
      }
    });
  });

  // Update chart when data changes
  $effect(() => {
    if (chart) {
      chart.data = data;
      chart.update('active');
    }
  });

  onDestroy(() => {
    if (chart) {
      chart.destroy();
      chart = null;
    }
  });
</script>

<div class="chart-container" style="height: {height}px;">
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  .chart-container {
    position: relative;
    width: 100%;
  }
</style>
