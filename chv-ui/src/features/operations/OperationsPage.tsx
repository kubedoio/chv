import { useQuery } from '@tanstack/react-query';
import { operationsApi } from '@/lib/api';
import { StatusBadge } from '@/components/status-badge/StatusBadge';
import { formatDate } from '@/lib/utils';
import { Activity, RefreshCw, AlertCircle } from 'lucide-react';
import type { Operation } from '@/types';

export function OperationsPage() {
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['operations'],
    queryFn: () => operationsApi.list(),
    refetchInterval: 5000,
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
        <p className="text-muted-foreground">Failed to load operations</p>
        <button
          onClick={() => refetch()}
          className="px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm"
        >
          Retry
        </button>
      </div>
    );
  }

  const operations = data?.items || [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Operations</h1>
          <p className="text-muted-foreground">
            {operations.length} operation{operations.length !== 1 ? 's' : ''}
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

      {operations.length === 0 ? (
        <div className="bg-card border border-border rounded-lg p-12 text-center">
          <Activity className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium">No operations found</h3>
          <p className="text-muted-foreground mt-1">
            Operations will appear here when you create or manage resources
          </p>
        </div>
      ) : (
        <div className="bg-card border border-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-muted">
                <tr>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Type</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Resource</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Status</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Progress</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Actor</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-muted-foreground">Started</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {operations.map((op: Operation) => (
                  <tr key={op.id} className="hover:bg-muted/50">
                    <td className="px-4 py-3">
                      <div className="font-medium capitalize">
                        {op.type.replace(/_/g, ' ')}
                      </div>
                      <div className="text-xs text-muted-foreground capitalize">
                        {op.category}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-sm">
                      {op.resource_type ? (
                        <>
                          <span className="capitalize">{op.resource_type}</span>
                          {op.resource_id && (
                            <div className="text-xs text-muted-foreground font-mono">
                              {op.resource_id.slice(0, 8)}
                            </div>
                          )}
                        </>
                      ) : (
                        <span className="text-muted-foreground">-</span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge status={op.status} />
                    </td>
                    <td className="px-4 py-3">
                      {op.progress_percent > 0 ? (
                        <div className="flex items-center gap-2">
                          <div className="w-24 h-2 bg-muted rounded-full overflow-hidden">
                            <div
                              className="h-full bg-primary"
                              style={{ width: `${op.progress_percent}%` }}
                            />
                          </div>
                          <span className="text-xs">{op.progress_percent}%</span>
                        </div>
                      ) : (
                        <span className="text-sm text-muted-foreground">-</span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-sm capitalize">
                      {op.actor_type}
                    </td>
                    <td className="px-4 py-3 text-sm text-muted-foreground">
                      {op.started_at ? formatDate(op.started_at) : formatDate(op.created_at)}
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
