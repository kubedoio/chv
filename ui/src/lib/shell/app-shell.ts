import {
	Activity,
	Blocks,
	Box,
	HardDrive,
	House,
	Image,
	Network,
	Server,
	Settings,
	Wrench
} from 'lucide-svelte';

export type ShellTone = 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown';
export type PageStateKey = 'loading' | 'empty' | 'error';

export interface NavItem {
	href: string;
	label: string;
	shortLabel: string;
	icon: typeof House;
}

export interface StateDefinition {
	title: string;
	description: string;
	hint: string;
}

export interface SummaryDefinition {
	label: string;
	value: string;
	note: string;
	tone?: ShellTone;
}

export interface BadgeDefinition {
	label: string;
	tone: ShellTone;
}

export interface PageDefinition {
	href: string;
	navLabel: string;
	shortLabel: string;
	title: string;
	eyebrow: string;
	description: string;
	icon: typeof House;
	badges: BadgeDefinition[];
	summary: SummaryDefinition[];
	focusAreas: string[];
	aliases?: string[];
	previewState?: PageStateKey;
	states: {
		loading: StateDefinition;
		empty: StateDefinition;
		error: StateDefinition;
	};
}

export interface NavGroup {
	label: string;
	items: NavItem[];
}

export const navigationGroups: NavGroup[] = [
	{
		label: 'Compute',
		items: [
			{ href: '/vms', label: 'Virtual Machines', shortLabel: 'VMs', icon: Box },
			{ href: '/images', label: 'Images / Templates', shortLabel: 'Images', icon: Image }
		]
	},
	{
		label: 'Infrastructure',
		items: [
			{ href: '/nodes', label: 'Nodes', shortLabel: 'Nodes', icon: Server },
			{
				href: '/clusters',
				label: 'Datacenters / Clusters',
				shortLabel: 'Clusters',
				icon: Blocks
			},
			{ href: '/networks', label: 'Networks', shortLabel: 'Networks', icon: Network },
			{ href: '/volumes', label: 'Volumes', shortLabel: 'Volumes', icon: HardDrive }
		]
	},
	{
		label: 'Operations',
		items: [
			{ href: '/tasks', label: 'Tasks', shortLabel: 'Tasks', icon: Activity },
			{ href: '/events', label: 'Events / Alerts', shortLabel: 'Events', icon: Activity },
			{
				href: '/maintenance',
				label: 'Maintenance / Upgrades',
				shortLabel: 'Maintenance',
				icon: Wrench
			}
		]
	},
	{
		label: 'Administration',
		items: [
			{ href: '/settings', label: 'Settings / Access', shortLabel: 'Settings', icon: Settings }
		]
	}
];

// Backward-compatible flat list derived from groups (Overview excluded — it lives above the groups)
export const navigationItems: NavItem[] = [
	{ href: '/', label: 'Overview', shortLabel: 'Overview', icon: House },
	...navigationGroups.flatMap((g) => g.items)
];

const pageDefinitions: PageDefinition[] = [
	{
		href: '/',
		navLabel: 'Overview',
		shortLabel: 'Overview',
		title: 'Overview',
		eyebrow: 'Fleet overview',
		description:
			'Fleet health, capacity pressure, active tasks, and alerts requiring attention.',
		icon: House,
		aliases: ['/metrics'],
		previewState: 'loading',
		badges: [
			{ label: 'Operational', tone: 'healthy' },
			{ label: 'Task-linked', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Infrastructure health',
				value: 'Nominal signals',
				note: 'Fleet, nodes, and VM readiness overview.',
				tone: 'healthy'
			},
			{
				label: 'Recent work',
				value: 'Active operations',
				note: 'Surface running tasks and recent failures.',
				tone: 'warning'
			},
			{
				label: 'Capacity usage',
				value: 'Resource pressure',
				note: 'CPU, memory, and storage utilization hotspots.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Fleet health summary',
			'Node readiness summary',
			'VM status summary',
			'Capacity summary',
			'Active alerts and recent failures',
			'Recent tasks and maintenance windows'
		],
		states: {
			loading: {
				title: 'Loading fleet overview',
				description:
					'Assembling fleet health, capacity, and recent activity from the control plane.',
				hint: 'Summary cards remain visible while data refreshes.'
			},
			empty: {
				title: 'No fleet data yet',
				description:
					'Fleet posture, alerts, and task summaries will appear once clusters and nodes are enrolled.',
				hint: 'Begin by enrolling a datacenter and cluster.'
			},
			error: {
				title: 'Fleet overview unavailable',
				description:
					'The control plane could not assemble the current fleet summary.',
				hint: 'Navigation remains available while the overview recovers.'
			}
		}
	},
	{
		href: '/clusters',
		navLabel: 'Datacenters / Clusters',
		shortLabel: 'Clusters',
		title: 'Datacenters / Clusters',
		eyebrow: 'Fleet topology',
		description:
			'Cluster inventory, readiness posture, and active work across datacenters.',
		icon: Blocks,
		badges: [
			{ label: 'Operational', tone: 'healthy' },
			{ label: 'Task-linked', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Primary view',
				value: 'Cluster roster',
				note: 'Datacenter, capacity posture, maintenance windows, and node counts.',
				tone: 'healthy'
			},
			{
				label: 'Operator focus',
				value: 'Pressure and imbalance',
				note: 'Surface degraded clusters, capacity hotspots, and active alerts first.',
				tone: 'warning'
			},
			{
				label: 'Related context',
				value: 'Nodes, tasks, events',
				note: 'Cross-link cluster detail into work history and incident context.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Datacenter and cluster inventory',
			'Readiness, maintenance, and version summaries',
			'Capacity hotspots',
			'Cluster-scoped tasks',
			'Cluster-scoped events and alerts'
		],
		states: {
			loading: {
				title: 'Loading cluster inventory',
				description:
					'Cluster rollups are being assembled from control-plane state, readiness, and capacity signals.',
				hint: 'Summary cards remain visible while the inventory refreshes.'
			},
			empty: {
				title: 'No clusters registered yet',
				description:
					'Register a datacenter and cluster with the control plane to populate this view.',
				hint: 'Cluster enrollment workflows will be available from the fleet setup page.'
			},
			error: {
				title: 'Cluster inventory unavailable',
				description:
					'The cluster rollups could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/nodes',
		navLabel: 'Nodes',
		shortLabel: 'Nodes',
		title: 'Nodes',
		eyebrow: 'Compute inventory',
		description:
			'Monitor node readiness, maintenance state, version skew, and infrastructure pressure across the fleet.',
		icon: Server,
		badges: [
			{ label: 'Operational', tone: 'healthy' },
			{ label: 'Maintenance-aware', tone: 'warning' }
		],
		summary: [
			{
				label: 'Primary table',
				value: 'Node roster',
				note: 'State, CPU, memory, storage, network health, and version belong in the first scan.',
				tone: 'healthy'
			},
			{
				label: 'Decision support',
				value: 'Maintenance posture',
				note: 'Operators should know immediately which nodes are draining, degraded, or isolated.',
				tone: 'warning'
			},
			{
				label: 'Cross-links',
				value: 'VMs, volumes, networks',
				note: 'Node detail tabs should connect directly to related resources and recent tasks.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Node name and cluster',
			'State, maintenance mode, and version',
			'CPU and memory pressure',
			'Storage summary and network health',
			'Node-scoped tasks and events'
		],
		states: {
			loading: {
				title: 'Loading node roster',
				description:
					'Node readiness, capacity, and maintenance signals are being assembled.',
				hint: 'Table structure remains visible while data refreshes.'
			},
			empty: {
				title: 'No nodes enrolled yet',
				description:
					'Compute hosts must register with the control plane to populate this view.',
				hint: 'Node enrollment workflows will be available from the fleet setup page.'
			},
			error: {
				title: 'Node inventory unavailable',
				description:
					'The node roster could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/vms',
		navLabel: 'Virtual Machines',
		shortLabel: 'VMs',
		title: 'Virtual Machines',
		eyebrow: 'Workload operations',
		description:
			'Give operators a fast path into power state, health, placement, and the last task touching each VM.',
		icon: Box,
		badges: [
			{ label: 'Operational', tone: 'healthy' },
			{ label: 'Task-linked', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Primary table',
				value: 'VM roster',
				note: 'Power state, health, CPU, memory, tags, and last task should scan in one pass.',
				tone: 'healthy'
			},
			{
				label: 'Operator need',
				value: 'Fast lifecycle actions',
				note: 'Start, stop, and investigate should route through visible task context.',
				tone: 'warning'
			},
			{
				label: 'Detail pattern',
				value: 'Summary then expert controls',
				note: 'Keep progressive depth: summary, config, tasks, events, related resources.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'VM name, node, and power state',
			'Health and placement',
			'CPU, memory, storage, and network counts',
			'Tags and last task',
			'Console/access, tasks, and events tabs'
		],
		states: {
			loading: {
				title: 'Loading virtual machine inventory',
				description:
					'VM state, health, and placement details are being assembled.',
				hint: 'Lifecycle actions remain visible while data refreshes.'
			},
			empty: {
				title: 'No virtual machines found',
				description:
					'Create or import a VM to populate this page.',
				hint: 'VM creation workflows will be available from the provisioning page.'
			},
			error: {
				title: 'VM inventory unavailable',
				description:
					'The VM roster could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/volumes',
		navLabel: 'Volumes',
		shortLabel: 'Volumes',
		title: 'Volumes',
		eyebrow: 'Storage inventory',
		description:
			'Track attached volumes, backend class, health, capacity, and the last task that changed storage state.',
		icon: HardDrive,
		aliases: ['/storage'],
		badges: [
			{ label: 'Operational', tone: 'healthy' },
			{ label: 'Policy-aware', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Primary table',
				value: 'Volume roster',
				note: 'Name, backend, attached VM, health, size, node, and policy should be first-class.',
				tone: 'healthy'
			},
			{
				label: 'Operational check',
				value: 'Attachment and drift',
				note: 'Operators should quickly detect misplaced or unhealthy storage attachments.',
				tone: 'warning'
			},
			{
				label: 'Follow-up surfaces',
				value: 'Tasks and events',
				note: 'Volume changes should always link into task history and recent failures.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Volume name and backend class',
			'Attached VM and placement',
			'Health, size, and policy',
			'Node affinity and recent work',
			'Task and event cross-links'
		],
		states: {
			loading: {
				title: 'Loading volume inventory',
				description:
					'Storage records are being assembled into operator-friendly volume summaries.',
				hint: 'Table structure remains visible while data refreshes.'
			},
			empty: {
				title: 'No volumes found',
				description:
					'Create or import a volume to populate this page.',
				hint: 'Volume creation workflows will be available from the storage page.'
			},
			error: {
				title: 'Volume inventory unavailable',
				description:
					'The volume roster could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/networks',
		navLabel: 'Networks',
		shortLabel: 'Networks',
		title: 'Networks',
		eyebrow: 'Connectivity model',
		description:
			'Monitor network scope, health, public exposure, and attached workloads without surfacing low-level internals.',
		icon: Network,
		previewState: 'error',
		badges: [
			{ label: 'Exposure visible', tone: 'warning' },
			{ label: 'Operational', tone: 'healthy' }
		],
		summary: [
			{
				label: 'Primary list',
				value: 'Network roster',
				note: 'Scope, health, attached VMs, and public exposure belong in the first scan.',
				tone: 'healthy'
			},
			{
				label: 'Operator concern',
				value: 'Public exposure',
				note: 'Surface risk states clearly without depending on color alone.',
				tone: 'warning'
			},
			{
				label: 'Follow-up context',
				value: 'Related VMs and tasks',
				note: 'Every network row should connect to affected workloads and recent operations.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Network name and scope',
			'Health and exposure state',
			'Attached workloads',
			'Recent operations',
			'Related tasks and events'
		],
		states: {
			loading: {
				title: 'Loading network inventory',
				description:
					'Network health and exposure details are being assembled.',
				hint: 'List structure remains visible while data refreshes.'
			},
			empty: {
				title: 'No networks found',
				description:
					'Create a network to populate this page.',
				hint: 'Network creation workflows will be available from the connectivity page.'
			},
			error: {
				title: 'Network inventory unavailable',
				description:
					'The network roster could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/images',
		navLabel: 'Images / Templates',
		shortLabel: 'Images',
		title: 'Images / Templates',
		eyebrow: 'Provisioning inputs',
		description:
			'Manage reusable images and templates that feed VM creation without mixing them into runtime inventory.',
		icon: Image,
		aliases: ['/templates'],
		badges: [
			{ label: 'Operational', tone: 'healthy' },
			{ label: 'Template-ready', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Primary split',
				value: 'Images and templates',
				note: 'Keep base images and reusable templates adjacent but visually distinct.',
				tone: 'healthy'
			},
			{
				label: 'Provisioning source',
				value: 'Baseline images',
				note: 'Managed boot images and reusable template definitions.',
				tone: 'warning'
			},
			{
				label: 'Workflow routing',
				value: 'Provisioning pipeline',
				note: 'Creation tasks route into the global task history.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Base images and templates',
			'Source metadata and compatibility',
			'Template readiness',
			'Last task and related workload actions'
		],
		states: {
			loading: {
				title: 'Loading images and templates',
				description:
					'Image metadata and template summaries are being assembled.',
				hint: 'Table structure remains visible while data refreshes.'
			},
			empty: {
				title: 'No images found',
				description:
					'Import a base image or define a template to populate this page.',
				hint: 'Image import workflows will be available from the provisioning page.'
			},
			error: {
				title: 'Image inventory unavailable',
				description:
					'The image roster could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/tasks',
		navLabel: 'Tasks',
		shortLabel: 'Tasks',
		title: 'Tasks',
		eyebrow: 'Operator work log',
		description:
			'Every mutating action should create visible task context with clear status, resource scope, and timestamps.',
		icon: Activity,
		aliases: ['/operations'],
		badges: [
			{ label: 'Operational', tone: 'healthy' },
			{ label: 'Async visible', tone: 'warning' }
		],
		summary: [
			{
				label: 'Primary table',
				value: 'Task center',
				note: 'Resource type, status, operation, actor, and time window should filter cleanly.',
				tone: 'healthy'
			},
			{
				label: 'Operator promise',
				value: 'No silent success',
				note: 'Long-running work should immediately route users into task context.',
				tone: 'warning'
			},
			{
				label: 'Key links',
				value: 'Resources and events',
				note: 'Each task should open related resources and recent event context.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Status and operation type',
			'Resource type and specific resource',
			'Actor and time window',
			'Task detail timeline',
			'Links to affected resources and events'
		],
		states: {
			loading: {
				title: 'Loading task center',
				description:
					'Long-running operations are being assembled into operator-readable status rows.',
				hint: 'Filters remain visible while data refreshes.'
			},
			empty: {
				title: 'No tasks match the current view',
				description:
					'Try widening the status, resource, or time window filters.',
				hint: 'The task center keeps filter state even when the result set is empty.'
			},
			error: {
				title: 'Task center unavailable',
				description:
					'The task list could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/events',
		navLabel: 'Events / Alerts',
		shortLabel: 'Events',
		title: 'Events / Alerts',
		eyebrow: 'Incident context',
		description:
			'Surface recent events and active alerts with severity, resource scope, and acknowledgement state.',
		icon: Activity,
		badges: [
			{ label: 'Severity-aware', tone: 'warning' },
			{ label: 'Operational', tone: 'healthy' }
		],
		summary: [
			{
				label: 'Primary list',
				value: 'Events and alerts',
				note: 'Severity, resource, and acknowledgement state should filter quickly.',
				tone: 'healthy'
			},
			{
				label: 'Operator concern',
				value: 'What changed recently?',
				note: 'The first scan should isolate failures and active alerts without a noisy feed.',
				tone: 'warning'
			},
			{
				label: 'Cross-links',
				value: 'Tasks and resources',
				note: 'Events should deepen context rather than become a dead-end log.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Severity and acknowledgement state',
			'Resource scope and state',
			'Time filters',
			'Active alerts vs historical events',
			'Direct links to tasks and affected resources'
		],
		states: {
			loading: {
				title: 'Loading events and alerts',
				description:
					'Event streams and alert state are being assembled into a scannable feed.',
				hint: 'Filters and table structure remain visible while data refreshes.'
			},
			empty: {
				title: 'No events match the current view',
				description:
					'Try widening the severity or state filters.',
				hint: 'Filters are URL-backed so a filtered view can be shared between operators.'
			},
			error: {
				title: 'Event history unavailable',
				description:
					'The event feed could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the tasks page for related context.'
			}
		}
	},
	{
		href: '/maintenance',
		navLabel: 'Maintenance / Upgrades',
		shortLabel: 'Maintenance',
		title: 'Maintenance / Upgrades',
		eyebrow: 'Change coordination',
		description:
			'Coordinate maintenance windows, draining, and upgrade visibility without hiding the related tasks and alerts.',
		icon: Wrench,
		aliases: ['/backup-jobs'],
		badges: [
			{ label: 'Change-aware', tone: 'warning' },
			{ label: 'Operational', tone: 'healthy' }
		],
		summary: [
			{
				label: 'Primary view',
				value: 'Maintenance schedule',
				note: 'Planned windows, affected resources, and progress should anchor the page.',
				tone: 'healthy'
			},
			{
				label: 'Operator need',
				value: 'Upgrade visibility',
				note: 'Make in-flight work and blast radius obvious before mutations ship.',
				tone: 'warning'
			},
			{
				label: 'Cross-links',
				value: 'Tasks, nodes, versions',
				note: 'Maintenance is never isolated from task history and resource health.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Planned maintenance windows',
			'Affected nodes and clusters',
			'Upgrade readiness and version drift',
			'In-flight maintenance tasks',
			'Maintenance-related alerts'
		],
		states: {
			loading: {
				title: 'Loading maintenance context',
				description:
					'Planned work, impact, and upgrade metadata are being assembled.',
				hint: 'Maintenance context remains visible while data refreshes.'
			},
			empty: {
				title: 'No maintenance windows scheduled',
				description:
					'There are no active or planned maintenance windows in the fleet.',
				hint: 'Scheduled maintenance and draining operations will appear here.'
			},
			error: {
				title: 'Maintenance status unavailable',
				description:
					'The maintenance schedule could not be loaded. Navigation remains available.',
				hint: 'Retry the request or check the events page for related alerts.'
			}
		}
	},
	{
		href: '/settings',
		navLabel: 'Settings / Access',
		shortLabel: 'Settings',
		title: 'Settings / Access',
		eyebrow: 'Operator controls',
		description:
			'Keep settings narrow, auditable, and aligned to the control-plane boundary rather than exposing backend internals.',
		icon: Settings,
		aliases: ['/quotas'],
		badges: [
			{ label: 'Auditable', tone: 'healthy' },
			{ label: 'Intentionally scoped', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Primary surface',
				value: 'Access and preferences',
				note: 'Scope MVP settings to operator-facing access and product behavior.',
				tone: 'healthy'
			},
			{
				label: 'Guardrail',
				value: 'No token leakage',
				note: 'Access flows should stay explicit, bounded, and audit-friendly.',
				tone: 'warning'
			},
			{
				label: 'Support integration',
				value: 'Diagnostic tools',
				note: 'Export logs and diagnostic bundles for assistance.',
				tone: 'unknown'
			}
		],
		focusAreas: [
			'Access and session controls',
			'Operator preferences',
			'Audit-friendly mutations',
			'Support and diagnostics hooks'
		],
		states: {
			loading: {
				title: 'Loading settings',
				description:
					'Operator-facing configuration and access metadata are being assembled.',
				hint: 'Settings structure remains visible while data refreshes.'
			},
			empty: {
				title: 'No settings to display',
				description:
					'Settings will appear here as the control plane surface expands.',
				hint: 'Current settings are intentionally scoped to essential operational information.'
			},
			error: {
				title: 'Settings unavailable',
				description:
					'The settings view could not be loaded. Navigation remains available.',
				hint: 'Retry the request once the control plane is reachable.'
			}
		}
	}
];

export function getTopLevelPageDefinitions(): PageDefinition[] {
	return pageDefinitions;
}

export function getPageDefinition(pathname: string): PageDefinition {
	const matched = pageDefinitions.find(
		(page) =>
			page.href !== '/' &&
			(pathname === page.href ||
				pathname.startsWith(`${page.href}/`) ||
				page.aliases?.some(
					(alias) => pathname === alias || pathname.startsWith(`${alias}/`)
				))
	);

	return matched ?? pageDefinitions[0];
}

export function getPrimaryStateDefinition(page: PageDefinition): StateDefinition {
	const previewState = page.previewState ?? 'empty';
	return page.states[previewState];
}
