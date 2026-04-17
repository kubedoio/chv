import type { NodeHealth, HealthAlert } from '$lib/api/types';

// Reactive store for node health alerts
export const healthAlerts = $state<HealthAlert[]>([]);

// Track processed status changes to avoid duplicate alerts
const processedChanges = new Set<string>();

/**
 * Add a new health alert
 */
export function addHealthAlert(alert: HealthAlert): void {
  // Create a unique key for this alert
  const alertKey = `${alert.node_id}-${alert.type}-${alert.timestamp}`;
  
  // Check if we've already processed this change
  if (processedChanges.has(alertKey)) {
    return;
  }
  
  // Add to processed set
  processedChanges.add(alertKey);
  
  // Limit the size of processed changes
  if (processedChanges.size > 100) {
    const firstKey = processedChanges.values().next().value;
    if (firstKey !== undefined) {
      processedChanges.delete(firstKey);
    }
  }
  
  const newAlert: HealthAlert = {
    ...alert,
    id: crypto.randomUUID(),
    dismissed: false
  };
  
  healthAlerts.unshift(newAlert);
  
  // Keep only the last 50 alerts
  if (healthAlerts.length > 50) {
    healthAlerts.pop();
  }
  
  // Auto-dismiss after 30 seconds for non-critical alerts
  if (alert.severity !== 'critical') {
    setTimeout(() => {
      if (newAlert.id) dismissHealthAlert(newAlert.id);
    }, 30000);
  }
}

/**
 * Dismiss a health alert by ID
 */
export function dismissHealthAlert(id: string): void {
  const index = healthAlerts.findIndex(a => a.id === id);
  if (index !== -1) {
    healthAlerts[index].dismissed = true;
    // Remove from array after animation
    setTimeout(() => {
      const idx = healthAlerts.findIndex(a => a.id === id);
      if (idx !== -1) {
        healthAlerts.splice(idx, 1);
      }
    }, 300);
  }
}

/**
 * Dismiss all health alerts
 */
export function dismissAllHealthAlerts(): void {
  healthAlerts.length = 0;
  processedChanges.clear();
}

/**
 * Check node health and generate alerts for status changes
 */
export function checkNodeHealthChange(
  previousHealth: Map<string, NodeHealth>,
  currentHealth: NodeHealth[]
): void {
  const now = new Date().toISOString();
  
  for (const health of currentHealth) {
    const previous = previousHealth.get(health.node_id);
    
    if (!previous) {
      // New node
      if (health.status === 'offline') {
        addHealthAlert({
          node_id: health.node_id,
          node_name: health.node_name,
          type: 'node_offline',
          severity: 'warning',
          message: `Node ${health.node_name} is offline`,
          timestamp: now
        });
      }
      continue;
    }
    
    // Check for status changes
    if (previous.status !== health.status) {
      if (health.status === 'offline') {
        addHealthAlert({
          node_id: health.node_id,
          node_name: health.node_name,
          type: 'node_offline',
          severity: 'critical',
          message: `Node ${health.node_name} went offline`,
          timestamp: now
        });
      } else if (previous.status === 'offline' && health.status === 'online') {
        addHealthAlert({
          node_id: health.node_id,
          node_name: health.node_name,
          type: 'node_online',
          severity: 'info',
          message: `Node ${health.node_name} is back online`,
          timestamp: now
        });
      } else if (health.status === 'maintenance') {
        addHealthAlert({
          node_id: health.node_id,
          node_name: health.node_name,
          type: 'node_maintenance',
          severity: 'warning',
          message: `Node ${health.node_name} entered maintenance mode`,
          timestamp: now
        });
      } else if (health.status === 'error') {
        addHealthAlert({
          node_id: health.node_id,
          node_name: health.node_name,
          type: 'node_error',
          severity: 'critical',
          message: `Node ${health.node_name} has an error condition`,
          timestamp: now
        });
      }
    }
    
    // Check for high resource usage
    if (health.metrics && previous.metrics) {
      const cpuPercent = health.metrics.cpu_percent ?? 0;
      const memTotal = health.metrics.memory_total_mb || 1; // prevent div by zero
      const memPercent = (health.metrics.memory_used_mb / memTotal) * 100;
      
      const prevCpuPercent = previous.metrics.cpu_percent ?? 0;
      const prevMemTotal = previous.metrics.memory_total_mb || 1;
      const prevMemPercent = (previous.metrics.memory_used_mb / prevMemTotal) * 100;
      
      if (cpuPercent >= 95 && prevCpuPercent < 95) {
        addHealthAlert({
          node_id: health.node_id,
          node_name: health.node_name,
          type: 'high_cpu',
          severity: 'warning',
          message: `Node ${health.node_name} CPU usage is critically high (${cpuPercent.toFixed(1)}%)`,
          timestamp: now
        });
      }
      
      if (memPercent >= 95 && prevMemPercent < 95) {
        addHealthAlert({
          node_id: health.node_id,
          node_name: health.node_name,
          type: 'high_memory',
          severity: 'warning',
          message: `Node ${health.node_name} memory usage is critically high (${memPercent.toFixed(1)}%)`,
          timestamp: now
        });
      }
    }
  }
}

/**
 * Get active (non-dismissed) alerts
 */
export function getActiveAlerts(): HealthAlert[] {
  return healthAlerts.filter(a => !a.dismissed);
}

/**
 * Get alerts for a specific node
 */
export function getNodeAlerts(nodeId: string): HealthAlert[] {
  return healthAlerts.filter(a => a.node_id === nodeId && !a.dismissed);
}

/**
 * Get critical alerts
 */
export function getCriticalAlerts(): HealthAlert[] {
  return healthAlerts.filter(a => a.severity === 'critical' && !a.dismissed);
}
