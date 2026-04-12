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
	() => import('./nodes/23')
];

export const server_loads = [];

export const dictionary = {
		"/": [2],
		"/backup-jobs": [3],
		"/events": [4],
		"/images": [5],
		"/install": [6],
		"/login": [7],
		"/metrics": [8],
		"/networks": [9],
		"/networks/[id]": [10],
		"/nodes": [11],
		"/nodes/[id]": [12],
		"/nodes/[id]/images": [13],
		"/nodes/[id]/networks": [14],
		"/nodes/[id]/storage": [15],
		"/nodes/[id]/vms": [16],
		"/operations": [17],
		"/quotas": [18],
		"/settings": [19],
		"/storage": [20],
		"/templates": [21],
		"/vms": [22],
		"/vms/[id]": [23]
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