import { cn } from '@/lib/utils';
import type { NodeState, VMActualState, ImageStatus, OperationStatus } from '@/types';

interface StatusBadgeProps {
  status: NodeState | VMActualState | ImageStatus | OperationStatus | string;
  className?: string;
}

const statusStyles: Record<string, string> = {
  // Node states
  online: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-100',
  degraded: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-100',
  offline: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-100',
  maintenance: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-100',
  
  // VM actual states
  provisioning: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-100',
  starting: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-100',
  running: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-100',
  stopping: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-100',
  stopped: 'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-100',
  deleting: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-100',
  error: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-100',
  unknown: 'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-100',
  
  // Image states
  importing: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-100',
  normalizing: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-100',
  ready: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-100',
  
  // Operation states
  pending: 'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-100',
  in_progress: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-100',
  completed: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-100',
  succeeded: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-100',
  failed: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-100',
  cancelled: 'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-100',
  
  // Generic
  active: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-100',
  inactive: 'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-100',
  present: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-100',
};

export function StatusBadge({ status, className }: StatusBadgeProps) {
  const style = statusStyles[status] || statusStyles.unknown;
  
  return (
    <span
      className={cn(
        'inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium',
        style,
        className
      )}
    >
      {status.replace(/_/g, ' ')}
    </span>
  );
}
