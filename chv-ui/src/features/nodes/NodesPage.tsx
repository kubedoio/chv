import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { nodesApi } from '@/lib/api';
import { StatusBadge } from '@/components/status-badge/StatusBadge';
import { formatDate } from '@/lib/utils';
import { Server, RefreshCw, AlertCircle } from 'lucide-react';
import type { Node } from '@/types';

export function NodesPage() {
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['nodes'],
    queryFn: () => nodesApi.list(),
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
        <p className="text-muted-foreground">Failed to load nodes</p>
        <button
          onClick={() => refetch()}
          className="px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm"
        >
          Retry
        </button>
      </div>
    );
  }

  const nodes = data?.items || [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Nodes</h1>
          <p className="text-muted-foreground">
            {nodes.length} node{nodes.length !== 1 ? 's' : ''} in cluster
          </p>
        </div>
        <button
          onClick={() => refetch()}
          className="p-2 rounded-md border border-border hover:bg-accent"
          title="Refresh"
        >
          <RefreshCw className="h-4 w-4" />
        </button>
      </div>

      {nodes.length === 0 ? (
        <div className="bg-card border border-border rounded-lg p-12 text-center">
          <Server className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium">No nodes found</h3>
          <p className="text-muted-foreground mt-1">
            Nodes must be registered using the CHV CLI or API
          </p>
        </div>
      ) : (
        <div className="bg-card border border-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-muted">
                <tr>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">
                    Hostname
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">
                    Management IP
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">
                    Status
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">
                    Resources
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">
                    Last Heartbeat
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {nodes.map((node: Node) => (
                  <tr key={node.id} className="hover:bg-muted/50">
                    <td className="px-4 py-3">
                      <div className="font-medium">{node.hostname}</div>
                      <div className="text-xs text-muted-foreground">{node.id.slice(0, 8)}</div>
                    </td>
                    <td className="px-4 py-3 text-sm font-mono">
                      {node.management_ip}
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge status={node.status} />
                      {node.maintenance_mode && (
                        <span className="ml-2 text-xs text-muted-foreground">(maintenance)</span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-sm">
                      {node.total_cpu_cores} cores / {Math.round(node.total_ram_mb / 1024)} GB
                    </td>
                    <td className="px-4 py-3 text-sm text-muted-foreground">
                      {node.last_heartbeat ? formatDate(node.last_heartbeat) : 'Never'}
                    </td>
                    <td className="px-4 py-3">
                      <Link
                        to={`/nodes/${node.id}`}
                        className="text-sm text-primary hover:underline"
                      >
                        View
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
