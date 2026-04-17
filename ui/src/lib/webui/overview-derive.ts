import type { OverviewResponse } from '$lib/bff/types';

export interface PostureChip {
	label: string;
	value: number;
	variant?: 'degraded' | 'warning' | 'failed' | 'healthy';
}

export function buildPostureChips(overview: OverviewResponse): PostureChip[] {
	return [
		{ label: 'Clusters', value: overview.clusters_total ?? 0 },
		{ label: 'Nodes', value: overview.nodes_total ?? 0 },
		{ label: 'VMs running', value: overview.vms_running ?? 0 },
		{
			label: 'Degraded',
			value: (overview.clusters_degraded ?? 0) + (overview.nodes_degraded ?? 0),
			variant:
				(overview.clusters_degraded ?? 0) + (overview.nodes_degraded ?? 0) > 0
					? 'degraded'
					: undefined
		},
		{
			label: 'Tasks',
			value: overview.active_tasks ?? 0,
			variant: (overview.active_tasks ?? 0) > 0 ? 'warning' : undefined
		},
		{
			label: 'Alerts',
			value: overview.unresolved_alerts ?? 0,
			variant: (overview.unresolved_alerts ?? 0) > 0 ? 'failed' : undefined
		}
	];
}

export interface AttentionItem {
	type: 'cluster' | 'node' | 'alert';
	title: string;
	detail: string;
	href: string;
}

export function buildAttentionItems(overview: OverviewResponse): AttentionItem[] {
	const clustersDegraded = overview.clusters_degraded ?? 0;
	const nodesDegraded = overview.nodes_degraded ?? 0;
	const unresolvedAlerts = overview.unresolved_alerts ?? 0;

	const items: AttentionItem[] = [
		...(clustersDegraded > 0
			? [
					{
						type: 'cluster' as const,
						title: `${clustersDegraded} cluster${clustersDegraded === 1 ? '' : 's'} degraded`,
						detail: 'Review cluster posture for pressure or version skew.',
						href: '/clusters'
					}
				]
			: []),
		...(nodesDegraded > 0
			? [
					{
						type: 'node' as const,
						title: `${nodesDegraded} node${nodesDegraded === 1 ? '' : 's'} degraded`,
						detail: 'Check node readiness and capacity pressure.',
						href: '/nodes'
					}
				]
			: []),
		...(unresolvedAlerts > 0
			? [
					{
						type: 'alert' as const,
						title: `${unresolvedAlerts} unresolved alert${unresolvedAlerts === 1 ? '' : 's'}`,
						detail: 'Alerts require operator inspection or acknowledgement.',
						href: '/events'
					}
				]
			: [])
	];
	return items.slice(0, 4);
}
