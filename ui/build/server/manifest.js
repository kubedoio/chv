const manifest = (() => {
function __memo(fn) {
	let value;
	return () => value ??= (value = fn());
}

return {
	appDir: "_app",
	appPath: "_app",
	assets: new Set([]),
	mimeTypes: {},
	_: {
		client: {start:"_app/immutable/entry/start.CKHqmyPF.js",app:"_app/immutable/entry/app.CVT8HFo-.js",imports:["_app/immutable/entry/start.CKHqmyPF.js","_app/immutable/chunks/BAvU7Hvn.js","_app/immutable/chunks/DcgFWB6C.js","_app/immutable/chunks/DhZmg6On.js","_app/immutable/entry/app.CVT8HFo-.js","_app/immutable/chunks/DcgFWB6C.js","_app/immutable/chunks/CZBekB2q.js","_app/immutable/chunks/CBxsaHUx.js","_app/immutable/chunks/DhZmg6On.js","_app/immutable/chunks/BJSa69G7.js","_app/immutable/chunks/DaHUxVSo.js","_app/immutable/chunks/CkBC4Ss-.js"],stylesheets:[],fonts:[],uses_env_dynamic_public:true},
		nodes: [
			__memo(() => import('./chunks/0-DYbOjoZ-.js')),
			__memo(() => import('./chunks/1-KniixCow.js')),
			__memo(() => import('./chunks/2-68DEBsR4.js')),
			__memo(() => import('./chunks/3-CmzeELkZ.js')),
			__memo(() => import('./chunks/4-DB6nhcA1.js')),
			__memo(() => import('./chunks/5-CWNfMo1q.js')),
			__memo(() => import('./chunks/6-CFXK9NsE.js')),
			__memo(() => import('./chunks/7-CMOvuNBM.js')),
			__memo(() => import('./chunks/8-BYCoZFfe.js')),
			__memo(() => import('./chunks/9-z6FP2Owt.js')),
			__memo(() => import('./chunks/10-DfuxfsFo.js')),
			__memo(() => import('./chunks/11-CnG6_dcY.js'))
		],
		remotes: {
			
		},
		routes: [
			{
				id: "/",
				pattern: /^\/$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 2 },
				endpoint: null
			},
			{
				id: "/images",
				pattern: /^\/images\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 3 },
				endpoint: null
			},
			{
				id: "/install",
				pattern: /^\/install\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 4 },
				endpoint: null
			},
			{
				id: "/login",
				pattern: /^\/login\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 5 },
				endpoint: null
			},
			{
				id: "/networks",
				pattern: /^\/networks\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 6 },
				endpoint: null
			},
			{
				id: "/operations",
				pattern: /^\/operations\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 7 },
				endpoint: null
			},
			{
				id: "/settings",
				pattern: /^\/settings\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 8 },
				endpoint: null
			},
			{
				id: "/storage",
				pattern: /^\/storage\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 9 },
				endpoint: null
			},
			{
				id: "/vms",
				pattern: /^\/vms\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 10 },
				endpoint: null
			},
			{
				id: "/vms/[id]",
				pattern: /^\/vms\/([^/]+?)\/?$/,
				params: [{"name":"id","optional":false,"rest":false,"chained":false}],
				page: { layouts: [0,], errors: [1,], leaf: 11 },
				endpoint: null
			}
		],
		prerendered_routes: new Set([]),
		matchers: async () => {
			
			return {  };
		},
		server_assets: {}
	}
}
})();

const prerendered = new Set([]);

const base = "";

export { base, manifest, prerendered };
//# sourceMappingURL=manifest.js.map
