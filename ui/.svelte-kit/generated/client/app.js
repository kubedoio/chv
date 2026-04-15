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
		"/backup-jobs": [3],
		"/clusters": [4],
		"/events": [5],
		"/images": [6],
		"/install": [7],
		"/login": [8],
		"/maintenance": [9],
		"/metrics": [10],
		"/networks": [11],
		"/networks/[id]": [12],
		"/nodes": [13],
		"/nodes/[id]": [14],
		"/nodes/[id]/images": [15],
		"/nodes/[id]/networks": [16],
		"/nodes/[id]/storage": [17],
		"/nodes/[id]/vms": [18],
		"/operations": [19],
		"/quotas": [20],
		"/settings": [21],
		"/storage": [22],
		"/tasks": [23],
		"/templates": [24],
		"/vms": [25],
		"/vms/[id]": [26],
		"/volumes": [27]
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