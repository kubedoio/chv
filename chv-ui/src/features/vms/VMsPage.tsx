import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { vmsApi } from '@/lib/api';
import { StatusBadge } from '@/components/status-badge/StatusBadge';
import { formatDate } from '@/lib/utils';
import { Cpu, RefreshCw, AlertCircle, Plus } from 'lucide-react';
import type { VM } from '@/types';

export function VMsPage() {
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['vms'],
    queryFn: () => vmsApi.list(),
    refetchInterval: 10000,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <RefreshCw className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-64 space-y-4">
        <AlertCircle className="h-12 w-12 text-destructive" />
        <p className="text-muted-foreground">Failed to load VMs</p>
        <button
          onClick={() => refetch()}
          className="px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm"
        >
          Retry
        </button>
      </div>
    );
  }

  const vms = data?.items || [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Virtual Machines</h1>
          <p className="text-muted-foreground">
            {vms.length} VM{vms.length !== 1 ? 's' : ''}
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => refetch()}
            className="p-2 rounded-md border border-border hover:bg-accent"
            title="Refresh"
          >
            <RefreshCw className="h-4 w-4" />
          </button>
          <Link
            to="/vms/new"
            className="flex items-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:bg-primary/90"
          >
            <Plus className="h-4 w-4" />
            Create VM
          </Link>
        </div>
      </div>

      {vms.length === 0 ? (
        <div className="bg-card border border-border rounded-lg p-12 text-center">
          <Cpu className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium">No VMs found</h3>
          <p className="text-muted-foreground mt-1">
            Create your first virtual machine
          </p>
          <Link
            to="/vms/new"
            className="inline-flex items-center gap-2 mt-4 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:bg-primary/90"
          >
            <Plus className="h-4 w-4" />
            Create VM
          </Link>
        </div>
      ) : (
        <div className="bg-card border border-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-muted">
                <tr>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Name</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">State</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">vCPU</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Memory</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Created</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {vms.map((vm: VM) => (
                  <tr key={vm.id} className="hover:bg-muted/50">
                    <td className="px-4 py-3">
                      <div className="font-medium">{vm.name}</div>
                      <div className="text-xs text-muted-foreground">{vm.id.slice(0, 8)}</div>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex flex-col gap-1">
                        <StatusBadge status={vm.actual_state} />
                        <span className="text-xs text-muted-foreground">
                          desired: {vm.desired_state}
                        </span>
                      </div>
                    </td>
                    <td className="px-4 py-3 text-sm">{vm.spec.cpu}</td>
                    <td className="px-4 py-3 text-sm">{vm.spec.memory_mb} MB</td>
                    <td className="px-4 py-3 text-sm text-muted-foreground">
                      {formatDate(vm.created_at)}
                    </td>
                    <td className="px-4 py-3">
                      <Link
                        to={`/vms/${vm.id}`}
                        className="text-sm text-primary hover:underline"
                      >
                        Manage
                      </Link>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
