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
	listVms: '/v1/vms',
	getVm: '/v1/vms/get',
	createVm: '/v1/vms/create',
	mutateVm: '/v1/vms/mutate',
	listTasks: '/v1/tasks',
	listClusters: '/v1/clusters',
	listNetworks: '/v1/networks',
	getNetwork: '/v1/networks/get',
	listVolumes: '/v1/volumes',
	getVolume: '/v1/volumes/get',
	mutateVolume: '/v1/volumes/mutate',
	listEvents: '/v1/events',
	listVmEvents: '/v1/vms/events',
	listImages: '/v1/images',
	getMaintenance: '/v1/maintenance',
	getSettings: '/v1/settings'
} as const;
