<script lang="ts">
  import { onMount } from 'svelte';
  import { Cpu, MemoryStick, HardDrive, Server, ArrowUp, ArrowDown } from 'lucide-svelte';
  import { createAPIClient } from '$lib/api/client';
  import type { VM } from '$lib/api/types';

  interface VMWithMetrics extends VM {
    cpuUsage?: number;
    memoryUsage?: number;
    diskUsage?: number;
    nodeName?: string;
  }

  const client = createAPIClient();

  let vms = $state<VMWithMetrics[]>([]);
  let loading = $state(true);
  let sortBy = $state<'cpu' | 'memory' | 'disk'>('cpu');
  let sortOrder = $state<'asc' | 'desc'>('desc');

  onMount(async () => {
    try {
      const [vmList, nodes] = await Promise.all([
        client.listVMs(),
        client.listNodes()
      ]);

      // Create a map of node IDs to names
      const nodeMap = new Map(nodes.map((n: any) => [n.id, n.name]));

      // Load metrics for running VMs
      const vmsWithMetrics = await Promise.all(
        vmList
          .filter((vm: VM) => vm.actual_state === 'running')
          .map(async (vm: VM) => {
            try {
              const metrics = await client.getVMMetrics(vm.id);
              return {
                ...vm,
                cpuUsage: metrics?.current?.cpu?.usage_percent || 0,
                memoryUsage: metrics?.current?.memory?.usage_percent || 0,
                diskUsage: metrics?.current?.disk?.read_bytes || 0,
                nodeName: nodeMap.get(vm.node_id) || 'Unknown'
              };
            } catch (e) {
              return {
                ...vm,
                cpuUsage: 0,
                memoryUsage: 0,
                diskUsage: 0,
                nodeName: nodeMap.get(vm.node_id) || 'Unknown'
              };
            }
          })
      );

      vms = vmsWithMetrics;
    } catch (e) {
      console.error('Failed to load VMs:', e);
    } finally {
      loading = false;
    }
  });

  let sortedVMs = $derived([...vms].sort((a, b) => {
    let aVal = 0, bVal = 0;
    switch (sortBy) {
      case 'cpu':
        aVal = a.cpuUsage || 0;
        bVal = b.cpuUsage || 0;
        break;
      case 'memory':
        aVal = a.memoryUsage || 0;
        bVal = b.memoryUsage || 0;
        break;
      case 'disk':
        aVal = a.diskUsage || 0;
        bVal = b.diskUsage || 0;
        break;
    }
    return sortOrder === 'desc' ? bVal - aVal : aVal - bVal;
  }).slice(0, 10));

  function setSort(column: 'cpu' | 'memory' | 'disk') {
    if (sortBy === column) {
      sortOrder = sortOrder === 'desc' ? 'asc' : 'desc';
    } else {
      sortBy = column;
      sortOrder = 'desc';
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  }

  function getUsageColor(percent: number): string {
    if (percent >= 80) return 'text-red-600';
    if (percent >= 60) return 'text-amber-600';
    return 'text-emerald-600';
  }

  function getUsageBarColor(percent: number): string {
    if (percent >= 80) return 'bg-red-500';
    if (percent >= 60) return 'bg-amber-500';
    return 'bg-emerald-500';
  }
</script>

<div class="bg-white rounded-lg border border-slate-200">
  <div class="p-6 border-b border-slate-100">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Server size={20} class="text-slate-500" />
        <h3 class="text-lg font-semibold text-slate-900">Top Resource Consumers</h3>
      </div>
      <span class="text-sm text-slate-500">
        {vms.length} running VMs
      </span>
    </div>
  </div>

  {#if loading}
    <div class="p-6 space-y-4">
      {#each Array(5) as _}
        <div class="animate-pulse flex items-center gap-4">
          <div class="w-10 h-10 bg-slate-200 rounded-lg"></div>
          <div class="flex-1">
            <div class="h-4 bg-slate-200 rounded w-32 mb-2"></div>
            <div class="h-3 bg-slate-100 rounded w-48"></div>
          </div>
        </div>
      {/each}
    </div>
  {:else if vms.length === 0}
    <div class="p-12 text-center">
      <Server size={48} class="mx-auto text-slate-300 mb-4" />
      <h4 class="text-lg font-medium text-slate-900 mb-1">No running VMs</h4>
      <p class="text-sm text-slate-500">Start some VMs to see resource consumption</p>
    </div>
  {:else}
    <div class="overflow-x-auto">
      <table class="w-full">
        <thead>
          <tr class="bg-slate-50">
            <th class="px-6 py-3 text-left text-xs font-medium text-slate-500 uppercase tracking-wider">
              VM
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-slate-500 uppercase tracking-wider cursor-pointer hover:text-slate-700" onclick={() => setSort('cpu')}>
              <div class="flex items-center gap-1">
                <Cpu size={14} />
                CPU
                {#if sortBy === 'cpu'}
                  {#if sortOrder === 'desc'}
                    <ArrowDown size={14} />
                  {:else}
                    <ArrowUp size={14} />
                  {/if}
                {/if}
              </div>
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-slate-500 uppercase tracking-wider cursor-pointer hover:text-slate-700" onclick={() => setSort('memory')}>
              <div class="flex items-center gap-1">
                <MemoryStick size={14} />
                Memory
                {#if sortBy === 'memory'}
                  {#if sortOrder === 'desc'}
                    <ArrowDown size={14} />
                  {:else}
                    <ArrowUp size={14} />
                  {/if}
                {/if}
              </div>
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-slate-500 uppercase tracking-wider cursor-pointer hover:text-slate-700" onclick={() => setSort('disk')}>
              <div class="flex items-center gap-1">
                <HardDrive size={14} />
                Disk I/O
                {#if sortBy === 'disk'}
                  {#if sortOrder === 'desc'}
                    <ArrowDown size={14} />
                  {:else}
                    <ArrowUp size={14} />
                  {/if}
                {/if}
              </div>
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-slate-500 uppercase tracking-wider">
              Node
            </th>
          </tr>
        </thead>
        <tbody class="divide-y divide-slate-100">
          {#each sortedVMs as vm}
            <tr class="hover:bg-slate-50 transition-colors">
              <td class="px-6 py-4">
                <div class="flex items-center gap-3">
                  <div class="w-2 h-2 rounded-full bg-emerald-500"></div>
                  <div>
                    <div class="font-medium text-slate-900">{vm.name}</div>
                    <div class="text-xs text-slate-500">{vm.vcpu} vCPU • {vm.memory_mb} MB</div>
                  </div>
                </div>
              </td>
              <td class="px-6 py-4">
                <div class="w-24">
                  <div class="flex items-center justify-between text-sm mb-1">
                    <span class={getUsageColor(vm.cpuUsage || 0)}>{(vm.cpuUsage || 0).toFixed(1)}%</span>
                  </div>
                  <div class="w-full bg-slate-100 rounded-full h-1.5">
                    <div class="{getUsageBarColor(vm.cpuUsage || 0)} h-1.5 rounded-full transition-all" style="width: {Math.min(100, vm.cpuUsage || 0)}%"></div>
                  </div>
                </div>
              </td>
              <td class="px-6 py-4">
                <div class="w-24">
                  <div class="flex items-center justify-between text-sm mb-1">
                    <span class={getUsageColor(vm.memoryUsage || 0)}>{(vm.memoryUsage || 0).toFixed(1)}%</span>
                  </div>
                  <div class="w-full bg-slate-100 rounded-full h-1.5">
                    <div class="{getUsageBarColor(vm.memoryUsage || 0)} h-1.5 rounded-full transition-all" style="width: {Math.min(100, vm.memoryUsage || 0)}%"></div>
                  </div>
                </div>
              </td>
              <td class="px-6 py-4">
                <div class="text-sm text-slate-600">
                  {formatBytes(vm.diskUsage || 0)}
                </div>
              </td>
              <td class="px-6 py-4">
                <span class="text-sm text-slate-600">{vm.nodeName}</span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
