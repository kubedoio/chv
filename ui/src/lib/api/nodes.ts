// Re-export node types from types.ts for convenience
export type { 
  Node, 
  NodeWithResources, 
  NodeResources, 
  NodeMetrics,
  CreateNodeInput,
  CreateNodeResponse,
  UpdateNodeInput,
  TreeNode,
  ResourceType 
} from './types';

import type { Node, TreeNode } from './types';

// Backward compatibility - generates a default node placeholder
// This is used by the UI when no nodes exist yet
export function getDefaultNode(): Node {
  return {
    id: 'placeholder',
    name: 'Datacenter Node',
    hostname: 'connecting...',
    ip_address: '...',
    status: 'online',
    is_local: false,
    capabilities: undefined,
    last_seen_at: new Date().toISOString(),
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString()
  };
}

// Tree navigation helper functions

/**
 * Generate navigation tree from nodes
 */
export function generateTreeNodes(nodes: Node[]): TreeNode[] {
  const nodeChildren: TreeNode[] = nodes.map(node => ({
    id: node.id,
    type: 'node',
    label: node.name,
    status: node.status,
    expanded: true,
    href: `/nodes/${node.id}`,
    children: [
      {
        id: `${node.id}-vms`,
        type: 'resource',
        label: 'Virtual Machines',
        icon: 'server',
        href: `/nodes/${node.id}/vms`,
        // Count will be populated from actual data
        badge: 0
      },
      {
        id: `${node.id}-images`,
        type: 'resource',
        label: 'Images',
        icon: 'image',
        href: `/nodes/${node.id}/images`,
        badge: 0
      },
      {
        id: `${node.id}-storage`,
        type: 'resource',
        label: 'Storage',
        icon: 'hardDrive',
        href: `/nodes/${node.id}/storage`,
        badge: 0
      },
      {
        id: `${node.id}-networks`,
        type: 'resource',
        label: 'Networks',
        icon: 'network',
        href: `/nodes/${node.id}/networks`,
        badge: 0
      }
    ]
  }));

  return [
    {
      id: 'datacenter',
      type: 'datacenter',
      label: 'Datacenter',
      expanded: true,
      href: '/',
      children: [
        {
          id: 'nodes-group',
          type: 'resource',
          label: 'Nodes',
          icon: 'server',
          href: '/nodes',
          badge: nodes.length,
          children: nodeChildren
        },
        {
          id: 'global-storage',
          type: 'resource',
          label: 'Storage',
          icon: 'hardDrive',
          href: '/storage'
        },
        {
          id: 'global-networks',
          type: 'resource',
          label: 'Networks',
          icon: 'network',
          href: '/networks'
        },
        {
          id: 'global-images',
          type: 'resource',
          label: 'Images',
          icon: 'image',
          href: '/images'
        }
      ]
    }
  ];
}

/**
 * Get status color class for a node
 */
export function getNodeStatusColor(status: string): string {
  switch (status) {
    case 'online':
      return 'text-green-500';
    case 'offline':
      return 'text-red-500';
    case 'maintenance':
      return 'text-orange-500';
    case 'error':
      return 'text-red-600';
    default:
      return 'text-slate-400';
  }
}

/**
 * Get status background class for a node
 */
export function getNodeStatusBg(status: string): string {
  switch (status) {
    case 'online':
      return 'bg-green-100';
    case 'offline':
      return 'bg-red-100';
    case 'maintenance':
      return 'bg-orange-100';
    case 'error':
      return 'bg-red-100';
    default:
      return 'bg-slate-100';
  }
}

/**
 * Format last seen timestamp to human readable string
 */
export function formatLastSeen(lastSeenAt?: string): string {
  if (!lastSeenAt) return 'Never';
  
  const date = new Date(lastSeenAt);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffSec < 60) return 'Just now';
  if (diffMin < 60) return `${diffMin}m ago`;
  if (diffHour < 24) return `${diffHour}h ago`;
  if (diffDay < 7) return `${diffDay}d ago`;
  return date.toLocaleDateString();
}
