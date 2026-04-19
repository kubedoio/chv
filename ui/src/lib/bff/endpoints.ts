/**
 * BFF HTTP endpoint mappings.
 *
 * The proto contracts (webui-bff.proto) define gRPC services. These paths
 * represent the agreed REST translation layer exposed by the BFF gateway.
 *
 * If the BFF is not yet running, set BFF_BASE_URL to a local stub or
 * leave it empty to enable client-only fallback behavior.
 */

export const BFFEndpoints = {
	overview: '/v1/overview',
	listNodes: '/v1/nodes',
	getNode: '/v1/nodes/get',
	mutateNode: '/v1/nodes/mutate',
	enrollNode: '/v1/nodes/enroll',
	listVms: '/v1/vms',
	getVm: '/v1/vms/get',
	getVmConsole: '/v1/vms/console',
	getVmConsoleUrl: '/v1/vms/console-url',
	createVm: '/v1/vms/create',
	mutateVm: '/v1/vms/mutate',
	deleteVm: '/v1/vms/delete',
	listTasks: '/v1/tasks',
	listClusters: '/v1/clusters',
	listNetworks: '/v1/networks',
	getNetwork: '/v1/networks/get',
	createNetwork: '/v1/networks/create',
	updateNetwork: '/v1/networks/update',
	deleteNetwork: '/v1/networks/delete',
	listVolumes: '/v1/volumes',
	getVolume: '/v1/volumes/get',
	mutateVolume: '/v1/volumes/mutate',
	listEvents: '/v1/events',
	listVmEvents: '/v1/vms/events',
	listImages: '/v1/images',
	importImage: '/v1/images/import',
	getMaintenance: '/v1/maintenance',
	getSettings: '/v1/settings'
} as const;
