import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { nodesApi, networksApi, storagePoolsApi, imagesApi, vmsApi } from '@/lib/api';
import { StatusBadge } from '@/components/status-badge/StatusBadge';
import { 
  Server, 
  Network, 
  HardDrive, 
  Image, 
  Cpu, 
  Plus,
  AlertCircle 
} from 'lucide-react';
import type { Node, VM } from '@/types';

interface StatCardProps {
  title: string;
  value: number | string;
  icon: React.ElementType;
  href: string;
  color: string;
}

function StatCard({ title, value, icon: IconComponent, href, color }: StatCardProps) {
  // IconComponent is used below
  return (
    <Link
      to={href}
      className="bg-card border border-border rounded-lg p-6 hover:border-primary/50 transition-colors"
    >
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm text-muted-foreground">{title}</p>
          <p className="text-3xl font-bold mt-2">{value}</p>
        </div>
        <div className={`p-3 rounded-lg ${color}`}>
          <IconComponent className="h-6 w-6" />
        </div>
      </div>
    </Link>
  );
}

interface QuickActionProps {
  label: string;
  href: string;
  icon: React.ElementType;
}

function QuickAction({ label, href }: QuickActionProps) {
  return (
    <Link
      to={href}
      className="flex items-center gap-2 p-3 rounded-md border border-border hover:bg-accent transition-colors"
    >
      <Plus className="h-4 w-4" />
      <span className="text-sm">{label}</span>
    </Link>
  );
}

export function DashboardPage() {
  const { data: nodes } = useQuery({
    queryKey: ['nodes'],
    queryFn: () => nodesApi.list(),
    refetchInterval: 15000,
  });

  const { data: networks } = useQuery({
    queryKey: ['networks'],
    queryFn: () => networksApi.list(),
    refetchInterval: 15000,
  });

  const { data: pools } = useQuery({
    queryKey: ['storage-pools'],
    queryFn: () => storagePoolsApi.list(),
    refetchInterval: 15000,
  });

  const { data: images } = useQuery({
    queryKey: ['images'],
    queryFn: () => imagesApi.list(),
    refetchInterval: 15000,
  });

  const { data: vms } = useQuery({
    queryKey: ['vms'],
    queryFn: () => vmsApi.list(),
    refetchInterval: 10000,
  });

  const nodeStats = {
    total: nodes?.items?.length || 0,
    online: nodes?.items?.filter((n: Node) => n.status === 'online').length || 0,
    degraded: nodes?.items?.filter((n: Node) => n.status === 'degraded').length || 0,
    offline: nodes?.items?.filter((n: Node) => n.status === 'offline').length || 0,
    maintenance: nodes?.items?.filter((n: Node) => n.status === 'maintenance').length || 0,
  };

  const vmStats = {
    total: vms?.items?.length || 0,
    running: vms?.items?.filter((v: VM) => v.actual_state === 'running').length || 0,
    stopped: vms?.items?.filter((v: VM) => v.actual_state === 'stopped').length || 0,
    error: vms?.items?.filter((v: VM) => v.actual_state === 'error').length || 0,
    provisioning: vms?.items?.filter((v: VM) => v.actual_state === 'provisioning').length || 0,
  };

  const hasErrors = vmStats.error > 0 || nodeStats.offline > 0 || nodeStats.degraded > 0;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-foreground">Dashboard</h1>
        <p className="text-muted-foreground">Overview of your CHV cluster</p>
      </div>

      {hasErrors && (
        <div className="flex items-start gap-2 p-4 rounded-md bg-destructive/10 border border-destructive/20">
          <AlertCircle className="h-5 w-5 text-destructive mt-0.5 shrink-0" />
          <div>
            <p className="font-medium text-destructive">Attention Required</p>
            <p className="text-sm text-destructive/80">
              {vmStats.error > 0 && `${vmStats.error} VM(s) in error state. `}
              {nodeStats.offline > 0 && `${nodeStats.offline} node(s) offline. `}
              {nodeStats.degraded > 0 && `${nodeStats.degraded} node(s) degraded. `}
            </p>
          </div>
        </div>
      )}

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          title="Nodes"
          value={nodeStats.total}
          icon={Server}
          href="/nodes"
          color="bg-blue-100 text-blue-600 dark:bg-blue-900 dark:text-blue-100"
        />
        <StatCard
          title="Networks"
          value={networks?.items?.length || 0}
          icon={Network}
          href="/networks"
          color="bg-purple-100 text-purple-600 dark:bg-purple-900 dark:text-purple-100"
        />
        <StatCard
          title="Storage Pools"
          value={pools?.items?.length || 0}
          icon={HardDrive}
          href="/storage-pools"
          color="bg-orange-100 text-orange-600 dark:bg-orange-900 dark:text-orange-100"
        />
        <StatCard
          title="Images"
          value={images?.items?.length || 0}
          icon={Image}
          href="/images"
          color="bg-green-100 text-green-600 dark:bg-green-900 dark:text-green-100"
        />
      </div>

      {/* VMs Section */}
      <div className="bg-card border border-border rounded-lg p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Cpu className="h-5 w-5 text-muted-foreground" />
            <h2 className="text-lg font-semibold">Virtual Machines</h2>
          </div>
          <Link
            to="/vms"
            className="text-sm text-primary hover:underline"
          >
            View all
          </Link>
        </div>
        
        <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
          <div className="text-center p-4 bg-muted rounded-lg">
            <p className="text-2xl font-bold">{vmStats.total}</p>
            <p className="text-sm text-muted-foreground">Total</p>
          </div>
          <div className="text-center p-4 bg-green-50 dark:bg-green-900/20 rounded-lg">
            <p className="text-2xl font-bold text-green-600">{vmStats.running}</p>
            <p className="text-sm text-muted-foreground">Running</p>
          </div>
          <div className="text-center p-4 bg-gray-50 dark:bg-gray-800/50 rounded-lg">
            <p className="text-2xl font-bold">{vmStats.stopped}</p>
            <p className="text-sm text-muted-foreground">Stopped</p>
          </div>
          <div className="text-center p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
            <p className="text-2xl font-bold text-blue-600">{vmStats.provisioning}</p>
            <p className="text-sm text-muted-foreground">Provisioning</p>
          </div>
          <div className="text-center p-4 bg-red-50 dark:bg-red-900/20 rounded-lg">
            <p className="text-2xl font-bold text-red-600">{vmStats.error}</p>
            <p className="text-sm text-muted-foreground">Error</p>
          </div>
        </div>
      </div>

      {/* Node Status */}
      {nodes?.items && nodes.items.length > 0 && (
        <div className="bg-card border border-border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">Node Status</h2>
          <div className="space-y-2">
            {nodes.items.slice(0, 5).map((node: Node) => (
              <div
                key={node.id}
                className="flex items-center justify-between p-3 bg-muted rounded-md"
              >
                <div className="flex items-center gap-3">
                  <Server className="h-4 w-4 text-muted-foreground" />
                  <div>
                    <p className="font-medium">{node.hostname}</p>
                    <p className="text-xs text-muted-foreground">{node.management_ip}</p>
                  </div>
                </div>
                <div className="flex items-center gap-4">
                  <span className="text-sm text-muted-foreground">
                    {node.total_cpu_cores} CPU / {Math.round(node.total_ram_mb / 1024)} GB RAM
                  </span>
                  <StatusBadge status={node.status} />
                </div>
              </div>
            ))}
          </div>
          {nodes.items.length > 5 && (
            <p className="text-center text-sm text-muted-foreground mt-4">
              +{nodes.items.length - 5} more nodes
            </p>
          )}
        </div>
      )}

      {/* Quick Actions */}
      <div className="bg-card border border-border rounded-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Quick Actions</h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <QuickAction label="Create Network" href="/networks" icon={Network} />
          <QuickAction label="Add Storage Pool" href="/storage-pools" icon={HardDrive} />
          <QuickAction label="Import Image" href="/images" icon={Image} />
          <QuickAction label="Create VM" href="/vms/new" icon={Cpu} />
        </div>
      </div>
    </div>
  );
}
