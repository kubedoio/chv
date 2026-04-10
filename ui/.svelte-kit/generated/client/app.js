export { matchers } from './matchers.js';

export const nodes = [
	() => import('./nodes/0'),
	() => import('./nodes/1'),
	() => import('./nodes/2'),
	() => import('./nodes/3'),
	() => import('./nodes/4'),
	() => import('./nodes/5'),
	() => import('./nodes/6'),
	() => import('./nodes/7'),
	() => import('./nodes/8'),
	() => import('./nodes/9'),
	() => import('./nodes/10'),
	() => import('./nodes/11'),
	() => import('./nodes/12'),
	() => import('./nodes/13'),
	() => import('./nodes/14'),
	() => import('./nodes/15'),
	() => import('./nodes/16'),
	() => import('./nodes/17'),
	() => import('./nodes/18'),
	() => import('./nodes/19'),
	() => import('./nodes/20'),
	() => import('./nodes/21'),
	() => import('./nodes/22'),
	() => import('./nodes/23'),
	() => import('./nodes/24'),
	() => import('./nodes/25'),
	() => import('./nodes/26'),
	() => import('./nodes/27')
];

export const server_loads = [];

export const dictionary = {
		"/": [2],
		"/events": [3],
		"/images": [4],
		"/install": [5],
		"/login": [6],
		"/metrics": [7],
		"/networks": [8],
		"/networks/[id]": [9],
		"/nodes": [10],
		"/nodes/[id]": [11],
		"/nodes/[id]/images": [12],
		"/nodes/[id]/networks": [13],
		"/nodes/[id]/storage": [14],
		"/nodes/[id]/vms": [15],
		"/operations": [16],
		"/quotas": [17],
		"/settings": [18],
		"/storage": [19],
		"/templates": [20],
		"/test/confirm-dialog": [21],
		"/test/forms": [22],
		"/test/modal": [23],
		"/test/skeletons": [24],
		"/test/stats-card": [25],
		"/vms": [26],
		"/vms/[id]": [27]
	};

export const hooks = {
	handleError: (({ error }) => { console.error(error) }),
	
	reroute: (() => {}),
	transport: {}
};

export const decoders = Object.fromEntries(Object.entries(hooks.transport).map(([k, v]) => [k, v.decode]));
export const encoders = Object.fromEntries(Object.entries(hooks.transport).map(([k, v]) => [k, v.encode]));

export const hash = false;

export const decode = (type, value) => decoders[type](value);

export { default as root } from '../root.js';