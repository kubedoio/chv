import { useQuery } from '@tanstack/react-query';
import { storagePoolsApi } from '@/lib/api';
import { StatusBadge } from '@/components/status-badge/StatusBadge';
import { formatBytes } from '@/lib/utils';
import { HardDrive, RefreshCw, AlertCircle, Plus } from 'lucide-react';
import type { StoragePool } from '@/types';

export function StoragePoolsPage() {
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['storage-pools'],
    queryFn: () => storagePoolsApi.list(),
    refetchInterval: 15000,
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
        <p className="text-muted-foreground">Failed to load storage pools</p>
        <button
          onClick={() => refetch()}
          className="px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm"
        >
          Retry
        </button>
      </div>
    );
  }

  const pools = data?.items || [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Storage Pools</h1>
          <p className="text-muted-foreground">
            {pools.length} pool{pools.length !== 1 ? 's' : ''} configured
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
          <button className="flex items-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:bg-primary/90">
            <Plus className="h-4 w-4" />
            Create Pool
          </button>
        </div>
      </div>

      {pools.length === 0 ? (
        <div className="bg-card border border-border rounded-lg p-12 text-center">
          <HardDrive className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium">No storage pools found</h3>
          <p className="text-muted-foreground mt-1">
            Create a storage pool to store VM disks
          </p>
        </div>
      ) : (
        <div className="bg-card border border-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-muted">
                <tr>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Name</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Type</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Path</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Capacity</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Status</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {pools.map((pool: StoragePool) => (
                  <tr key={pool.id} className="hover:bg-muted/50">
                    <td className="px-4 py-3">
                      <div className="font-medium">{pool.name}</div>
                    </td>
                    <td className="px-4 py-3 text-sm capitalize">{pool.pool_type}</td>
                    <td className="px-4 py-3 text-sm font-mono truncate max-w-xs">{pool.path_or_export}</td>
                    <td className="px-4 py-3 text-sm">
                      {pool.capacity_bytes ? formatBytes(pool.capacity_bytes) : 'Unknown'}
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge status={pool.status} />
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
