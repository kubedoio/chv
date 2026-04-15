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

export const navigationItems: NavItem[] = [
	{ href: '/', label: 'Overview', shortLabel: 'Overview', icon: House },
	{
		href: '/clusters',
		label: 'Datacenters / Clusters',
		shortLabel: 'Clusters',
		icon: Blocks
	},
	{ href: '/nodes', label: 'Nodes', shortLabel: 'Nodes', icon: Server },
	{ href: '/vms', label: 'Virtual Machines', shortLabel: 'VMs', icon: Box },
	{ href: '/volumes', label: 'Volumes', shortLabel: 'Volumes', icon: HardDrive },
	{ href: '/networks', label: 'Networks', shortLabel: 'Networks', icon: Network },
	{
		href: '/images',
		label: 'Images / Templates',
		shortLabel: 'Images',
		icon: Image
	},
	{ href: '/tasks', label: 'Tasks', shortLabel: 'Tasks', icon: Activity },
	{ href: '/events', label: 'Events / Alerts', shortLabel: 'Events', icon: Activity },
	{
		href: '/maintenance',
		label: 'Maintenance / Upgrades',
		shortLabel: 'Maintenance',
		icon: Wrench
	},
	{ href: '/settings', label: 'Settings / Access', shortLabel: 'Settings', icon: Settings }
];

const pageDefinitions: PageDefinition[] = [
	{
		href: '/',
		navLabel: 'Overview',
		shortLabel: 'Overview',
		title: 'Overview',
		eyebrow: 'Fleet control surface',
		description:
			'Cluster-first summary of health, capacity, recent failures, and operator work in progress.',
		icon: House,
		aliases: ['/metrics'],
		previewState: 'loading',
		badges: [
			{ label: 'Cluster-first', tone: 'healthy' },
			{ label: 'Task-visible', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Health strip',
				value: 'Fleet, nodes, VMs',
				note: 'Scan high-level readiness before opening resource lists.',
				tone: 'healthy'
			},
			{
				label: 'Recent work',
				value: 'Tasks and failures',
				note: 'Surface active operations and the last failed workflows.',
				tone: 'warning'
			},
			{
				label: 'Capacity pulse',
				value: 'CPU, memory, storage',
				note: 'Reserve space for real capacity cards once BFF view models land.',
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
					'The BFF is shaping health, capacity, and recent activity into an operator-first summary.',
				hint: 'Keep summary cards visible while data loads so the shell never feels blank.'
			},
			empty: {
				title: 'No infrastructure enrolled yet',
				description:
					'Once clusters and nodes are connected, overview cards should populate with fleet status and active work.',
				hint: 'Primary empty CTA should guide operators to cluster or node enrollment.'
			},
			error: {
				title: 'Overview data is unavailable',
				description:
					'If the summary cannot be shaped, preserve shell navigation and show clear retry and task context.',
				hint: 'Failure messaging should mention the BFF boundary rather than leaking backend topology.'
			}
		}
	},
	{
		href: '/clusters',
		navLabel: 'Datacenters / Clusters',
		shortLabel: 'Clusters',
		title: 'Datacenters / Clusters',
		eyebrow: 'Infrastructure topology',
		description:
			'Organize fleet inventory by datacenter and cluster before drilling into node-level operations.',
		icon: Blocks,
		badges: [
			{ label: 'Topology-first', tone: 'healthy' },
			{ label: 'Reusable shell', tone: 'unknown' }
		],
		summary: [
			{
				label: 'Primary list',
				value: 'Cluster roster',
				note: 'Datacenter, capacity posture, maintenance windows, and node counts.',
				tone: 'healthy'
			},
			{
				label: 'Operator question',
				value: 'Where is pressure?',
				note: 'The first slice should make imbalance obvious before detail navigation.',
				tone: 'warning'
			},
			{
				label: 'Related resources',
				value: 'Nodes, tasks, events',
				note: 'Cluster detail should cross-link directly into work and incident context.',
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
					'The BFF is assembling cluster rollups from control-plane state, readiness, and capacity signals.',
				hint: 'Cluster cards should feel like operational summaries, not raw tree nodes.'
			},
			empty: {
				title: 'No clusters defined',
				description:
					'Use this state before the first datacenter or cluster is registered with the control plane.',
				hint: 'Future CTA can point to cluster import or node enrollment workflows.'
			},
			error: {
				title: 'Cluster topology could not be shaped',
				description:
					'Keep the navigation stable and show a bounded failure when cluster rollups fail to load.',
				hint: 'Avoid leaking internal graph assembly details into the browser.'
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
			{ label: 'Readiness visible', tone: 'healthy' },
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
					'The BFF is shaping node readiness, capacity, and maintenance signals into a list view.',
				hint: 'Skeleton rows should hint at table density without becoming visually noisy.'
			},
			empty: {
				title: 'No nodes enrolled',
				description:
					'This state appears before any compute hosts register with the control plane.',
				hint: 'The empty state should guide enrollment, not dump technical prerequisites.'
			},
			error: {
				title: 'Node inventory unavailable',
				description:
					'When node view models fail, preserve filters and routing so operators can recover quickly.',
				hint: 'Show retry context and the last successful sync when real data lands.'
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
			{ label: 'Lifecycle first', tone: 'healthy' },
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
					'The BFF is shaping VM state, health, and placement details into a workload-first table.',
				hint: 'Keep lifecycle actions visible but disabled while data is still arriving.'
			},
			empty: {
				title: 'No virtual machines found',
				description:
					'When no workloads exist yet, the page should still explain the expected next operator action.',
				hint: 'Use realistic copy about creating or importing the first VM.'
			},
			error: {
				title: 'VM inventory could not be loaded',
				description:
					'If workload view models fail, preserve surrounding context so operators can pivot to tasks or events.',
				hint: 'Pair retry affordances with a short explanation of what data is missing.'
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
			{ label: 'Health legible', tone: 'healthy' },
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
					'The BFF is shaping storage records into operator-friendly volume summaries.',
				hint: 'Table skeletons should keep the high-density storage workflow legible.'
			},
			empty: {
				title: 'No volumes discovered',
				description:
					'Use this state when storage backends exist but no managed volumes are currently visible.',
				hint: 'Future CTA can route to volume creation or import flows.'
			},
			error: {
				title: 'Volume data is unavailable',
				description:
					'If storage view models fail, keep navigation and task context visible while offering a retry.',
				hint: 'Storage failures should remain concise and action-oriented.'
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
			{ label: 'Control-plane shaped', tone: 'healthy' }
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
					'The BFF is shaping network health and exposure details into a concise operator view.',
				hint: 'Keep lists stable while filters and count chips load.'
			},
			empty: {
				title: 'No managed networks found',
				description:
					'This state should explain what kinds of networks appear here once configured.',
				hint: 'Future CTA can route to safe network creation flows.'
			},
			error: {
				title: 'Network view models failed to load',
				description:
					'When connectivity inventory is unavailable, hold the shell steady and provide recovery context.',
				hint: 'Show retry context, last refresh time, and task/event escape hatches.'
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
			{ label: 'Provisioning source', tone: 'healthy' },
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
				label: 'Operator task',
				value: 'Source of new VMs',
				note: 'The first slice should clarify what can be imported, cloned, or reused.',
				tone: 'warning'
			},
			{
				label: 'Follow-up flow',
				value: 'Create VM from source',
				note: 'Creation should route straight into visible task context once mutations exist.',
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
					'The BFF is shaping image metadata and template summaries for VM provisioning workflows.',
				hint: 'Use dense list skeletons rather than oversized cards.'
			},
			empty: {
				title: 'No reusable sources available',
				description:
					'This state should guide operators toward importing a base image or defining the first template.',
				hint: 'Keep the copy grounded in provisioning workflows, not generic asset language.'
			},
			error: {
				title: 'Provisioning sources are unavailable',
				description:
					'If image or template summaries fail, preserve the page frame and explain what data is missing.',
				hint: 'Failure copy should stay product-facing and operationally trustworthy.'
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
			{ label: 'First-class tasks', tone: 'healthy' },
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
					'The BFF is shaping long-running operations into operator-readable status and timeline rows.',
				hint: 'Filters should remain visible during loading to reinforce task-centric workflows.'
			},
			empty: {
				title: 'No tasks in the selected window',
				description:
					'When there is no recent work, keep the timeline frame stable and explain what will show up here.',
				hint: 'This state should feel calm, not alarming.'
			},
			error: {
				title: 'Task history is unavailable',
				description:
					'If task shaping fails, tell the operator that mutation context cannot be displayed right now.',
				hint: 'Offer retry plus a path into events for adjacent context.'
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
			{ label: 'Alert-linked', tone: 'healthy' }
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
					'The BFF is shaping event streams and alert state into a scannable operator feed.',
				hint: 'Keep filters and table structure stable while new records arrive.'
			},
			empty: {
				title: 'No active alerts in this view',
				description:
					'This state should reassure operators while still explaining how filters affect the timeline.',
				hint: 'Acknowledge the selected severity or time window when data is absent.'
			},
			error: {
				title: 'Event history is temporarily unavailable',
				description:
					'If event shaping fails, maintain the page frame and explain how operators can retry safely.',
				hint: 'Failure copy should not expose internal streaming or storage details.'
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
			{ label: 'Planned work', tone: 'unknown' }
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
					'The BFF is shaping planned work, impact, and upgrade metadata into operator-friendly views.',
				hint: 'Keep planned-change context legible even when details are still loading.'
			},
			empty: {
				title: 'No maintenance windows scheduled',
				description:
					'Use this state when there is no planned work or upgrade campaign in the selected scope.',
				hint: 'The copy should feel operational and calm, not like an error.'
			},
			error: {
				title: 'Maintenance status could not be loaded',
				description:
					'If planned-work data is unavailable, preserve surrounding shell context and explain the impact clearly.',
				hint: 'Operators should understand whether scheduling or just visibility is affected.'
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
			{ label: 'Low surface area', tone: 'unknown' }
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
				label: 'Future depth',
				value: 'Support workflows',
				note: 'Diagnostic export and support tools can land later without widening the shell.',
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
				title: 'Loading settings context',
				description:
					'The BFF is shaping operator-facing configuration and access metadata for safe presentation.',
				hint: 'Keep settings lists structured so future forms can slot in without shell changes.'
			},
			empty: {
				title: 'No configurable preferences yet',
				description:
					'This state is expected early in MVP while access and preference surfaces stay intentionally narrow.',
				hint: 'Explain the limited surface area as a product decision, not missing work.'
			},
			error: {
				title: 'Settings could not be loaded',
				description:
					'If access or preference data fails, preserve the shell and keep failure messaging bounded.',
				hint: 'Settings errors should stay precise and avoid backend implementation details.'
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
