import { useParams, Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { vmsApi } from '@/lib/api';
import { StatusBadge } from '@/components/status-badge/StatusBadge';
import { formatDate } from '@/lib/utils';
import { ArrowLeft, Cpu, Power, RotateCcw, Trash2, RefreshCw, AlertCircle, Terminal } from 'lucide-react';

export function VMDetailPage() {
  const { id } = useParams<{ id: string }>();
  
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['vm', id],
    queryFn: () => vmsApi.get(id!),
    enabled: !!id,
    refetchInterval: 5000,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <RefreshCw className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="space-y-4">
        <Link to="/vms" className="flex items-center gap-2 text-sm text-primary hover:underline">
          <ArrowLeft className="h-4 w-4" />
          Back to VMs
        </Link>
        <div className="flex flex-col items-center justify-center h-64 space-y-4">
          <AlertCircle className="h-12 w-12 text-destructive" />
          <p className="text-muted-foreground">VM not found or failed to load</p>
          <button
            onClick={() => refetch()}
            className="px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  const vm = data;

  const handleStart = async () => {
    try {
      await vmsApi.start(vm.id);
    } catch (err) {
      console.error('Failed to start VM:', err);
    }
  };

  const handleStop = async () => {
    try {
      await vmsApi.stop(vm.id);
    } catch (err) {
      console.error('Failed to stop VM:', err);
    }
  };

  const handleReboot = async () => {
    try {
      await vmsApi.reboot(vm.id);
    } catch (err) {
      console.error('Failed to reboot VM:', err);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <Link to="/vms" className="p-2 rounded-md hover:bg-accent">
            <ArrowLeft className="h-5 w-5" />
          </Link>
          <div>
            <h1 className="text-2xl font-bold text-foreground">{vm.name}</h1>
            <p className="text-muted-foreground">{vm.id}</p>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => refetch()}
            className="p-2 rounded-md border border-border hover:bg-accent"
            title="Refresh"
          >
            <RefreshCw className="h-4 w-4" />
          </button>
          {vm.actual_state === 'running' && (
            <button
              className="flex items-center gap-2 px-4 py-2 border border-border rounded-md text-sm font-medium hover:bg-accent"
            >
              <Terminal className="h-4 w-4" />
              Console
            </button>
          )}
        </div>
      </div>

      {/* Status Overview */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-card border border-border rounded-lg p-4">
          <p className="text-sm text-muted-foreground">Actual State</p>
          <div className="mt-2">
            <StatusBadge status={vm.actual_state} />
          </div>
        </div>
        <div className="bg-card border border-border rounded-lg p-4">
          <p className="text-sm text-muted-foreground">Desired State</p>
          <p className="text-lg font-medium mt-2 capitalize">{vm.desired_state}</p>
        </div>
        <div className="bg-card border border-border rounded-lg p-4">
          <p className="text-sm text-muted-foreground">Placement</p>
          <p className="text-lg font-medium mt-2 capitalize">{vm.placement_status}</p>
        </div>
        <div className="bg-card border border-border rounded-lg p-4">
          <p className="text-sm text-muted-foreground">Node</p>
          <p className="text-lg font-medium mt-2 font-mono truncate">
            {vm.node_id ? vm.node_id.slice(0, 8) : 'Not placed'}
          </p>
        </div>
      </div>

      {/* Compute Specs */}
      <div className="bg-card border border-border rounded-lg p-6">
        <div className="flex items-center gap-2 mb-4">
          <Cpu className="h-5 w-5 text-muted-foreground" />
          <h2 className="text-lg font-semibold">Compute</h2>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <dt className="text-sm text-muted-foreground">vCPUs</dt>
            <dd className="text-lg font-medium">{vm.spec.cpu}</dd>
          </div>
          <div>
            <dt className="text-sm text-muted-foreground">Memory</dt>
            <dd className="text-lg font-medium">{vm.spec.memory_mb} MB</dd>
          </div>
        </div>
      </div>

      {/* Actions */}
      <div className="bg-card border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Actions</h2>
        <div className="flex flex-wrap gap-3">
          {vm.actual_state !== 'running' && vm.actual_state !== 'starting' && (
            <button
              onClick={handleStart}
              className="flex items-center gap-2 px-4 py-2 bg-green-600 text-white rounded-md text-sm font-medium hover:bg-green-700"
            >
              <Power className="h-4 w-4" />
              Start
            </button>
          )}
          {vm.actual_state === 'running' && (
            <button
              onClick={handleStop}
              className="flex items-center gap-2 px-4 py-2 bg-yellow-600 text-white rounded-md text-sm font-medium hover:bg-yellow-700"
            >
              <Power className="h-4 w-4" />
              Stop
            </button>
          )}
          {vm.actual_state === 'running' && (
            <button
              onClick={handleReboot}
              className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700"
            >
              <RotateCcw className="h-4 w-4" />
              Reboot
            </button>
          )}
          <button
            className="flex items-center gap-2 px-4 py-2 border border-destructive text-destructive rounded-md text-sm font-medium hover:bg-destructive hover:text-destructive-foreground"
          >
            <Trash2 className="h-4 w-4" />
            Delete
          </button>
        </div>
      </div>

      {/* Timestamps */}
      <div className="bg-card border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Timeline</h2>
        <dl className="space-y-3">
          <div className="flex justify-between">
            <dt className="text-sm text-muted-foreground">Created</dt>
            <dd className="text-sm">{formatDate(vm.created_at)}</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-sm text-muted-foreground">Last Updated</dt>
            <dd className="text-sm">{formatDate(vm.updated_at)}</dd>
          </div>
        </dl>
      </div>
    </div>
  );
}
