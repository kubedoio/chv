import { useParams, Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { nodesApi } from '@/lib/api';
import { StatusBadge } from '@/components/status-badge/StatusBadge';
import { formatDate } from '@/lib/utils';
import { ArrowLeft, Server, RefreshCw, AlertCircle } from 'lucide-react';

export function NodeDetailPage() {
  const { id } = useParams<{ id: string }>();
  
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['node', id],
    queryFn: () => nodesApi.get(id!),
    enabled: !!id,
    refetchInterval: 15000,
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
        <Link to="/nodes" className="flex items-center gap-2 text-sm text-primary hover:underline">
          <ArrowLeft className="h-4 w-4" />
          Back to nodes
        </Link>
        <div className="flex flex-col items-center justify-center h-64 space-y-4">
          <AlertCircle className="h-12 w-12 text-destructive" />
          <p className="text-muted-foreground">Node not found or failed to load</p>
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

  const node = data;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link to="/nodes" className="p-2 rounded-md hover:bg-accent">
          <ArrowLeft className="h-5 w-5" />
        </Link>
        <div>
          <h1 className="text-2xl font-bold text-foreground">{node.hostname}</h1>
          <p className="text-muted-foreground">{node.id}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Identity Card */}
        <div className="bg-card border border-border rounded-lg p-6">
          <div className="flex items-center gap-2 mb-4">
            <Server className="h-5 w-5 text-muted-foreground" />
            <h2 className="text-lg font-semibold">Identity</h2>
          </div>
          <dl className="space-y-3">
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Hostname</dt>
              <dd className="text-sm font-medium">{node.hostname}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Management IP</dt>
              <dd className="text-sm font-mono">{node.management_ip}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Status</dt>
              <dd><StatusBadge status={node.status} /></dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Maintenance Mode</dt>
              <dd className="text-sm">{node.maintenance_mode ? 'Yes' : 'No'}</dd>
            </div>
          </dl>
        </div>

        {/* Resources Card */}
        <div className="bg-card border border-border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">Resources</h2>
          <dl className="space-y-3">
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Total CPU Cores</dt>
              <dd className="text-sm font-medium">{node.total_cpu_cores}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Total RAM</dt>
              <dd className="text-sm font-medium">{Math.round(node.total_ram_mb / 1024)} GB</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Allocatable CPU</dt>
              <dd className="text-sm font-medium">{node.allocatable_cpu_cores} cores</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Allocatable RAM</dt>
              <dd className="text-sm font-medium">{Math.round(node.allocatable_ram_mb / 1024)} GB</dd>
            </div>
          </dl>
        </div>

        {/* Heartbeat Card */}
        <div className="bg-card border border-border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">Heartbeat</h2>
          <dl className="space-y-3">
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Last Heartbeat</dt>
              <dd className="text-sm">{node.last_heartbeat ? formatDate(node.last_heartbeat) : 'Never'}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Created</dt>
              <dd className="text-sm">{formatDate(node.created_at)}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Updated</dt>
              <dd className="text-sm">{formatDate(node.updated_at)}</dd>
            </div>
          </dl>
        </div>

        {/* Versions Card */}
        <div className="bg-card border border-border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">Versions</h2>
          <dl className="space-y-3">
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Agent Version</dt>
              <dd className="text-sm font-medium">{node.agent_version || 'Unknown'}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-sm text-muted-foreground">Hypervisor Version</dt>
              <dd className="text-sm font-medium">{node.hypervisor_version || 'Unknown'}</dd>
            </div>
          </dl>
        </div>
      </div>

      {/* Actions */}
      <div className="bg-card border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Actions</h2>
        <div className="flex gap-4">
          <button
            className="px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:bg-primary/90 disabled:opacity-50"
            disabled={node.maintenance_mode}
          >
            Enter Maintenance
          </button>
          <button
            className="px-4 py-2 border border-border rounded-md text-sm font-medium hover:bg-accent disabled:opacity-50"
            disabled={!node.maintenance_mode}
          >
            Leave Maintenance
          </button>
        </div>
      </div>
    </div>
  );
}
