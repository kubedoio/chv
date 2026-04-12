
// this file is generated — do not edit it


declare module "svelte/elements" {
	export interface HTMLAttributes<T> {
		'data-sveltekit-keepfocus'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-noscroll'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-preload-code'?:
			| true
			| ''
			| 'eager'
			| 'viewport'
			| 'hover'
			| 'tap'
			| 'off'
			| undefined
			| null;
		'data-sveltekit-preload-data'?: true | '' | 'hover' | 'tap' | 'off' | undefined | null;
		'data-sveltekit-reload'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-replacestate'?: true | '' | 'off' | undefined | null;
	}
}

export {};


declare module "$app/types" {
	type MatcherParam<M> = M extends (param : string) => param is (infer U extends string) ? U : string;

	export interface AppTypes {
		RouteId(): "/" | "/backup-jobs" | "/events" | "/images" | "/install" | "/login" | "/metrics" | "/networks" | "/networks/[id]" | "/nodes" | "/nodes/[id]" | "/nodes/[id]/images" | "/nodes/[id]/networks" | "/nodes/[id]/storage" | "/nodes/[id]/vms" | "/operations" | "/quotas" | "/settings" | "/storage" | "/templates" | "/vms" | "/vms/[id]";
		RouteParams(): {
			"/networks/[id]": { id: string };
			"/nodes/[id]": { id: string };
			"/nodes/[id]/images": { id: string };
			"/nodes/[id]/networks": { id: string };
			"/nodes/[id]/storage": { id: string };
			"/nodes/[id]/vms": { id: string };
			"/vms/[id]": { id: string }
		};
		LayoutParams(): {
			"/": { id?: string };
			"/backup-jobs": Record<string, never>;
			"/events": Record<string, never>;
			"/images": Record<string, never>;
			"/install": Record<string, never>;
			"/login": Record<string, never>;
			"/metrics": Record<string, never>;
			"/networks": { id?: string };
			"/networks/[id]": { id: string };
			"/nodes": { id?: string };
			"/nodes/[id]": { id: string };
			"/nodes/[id]/images": { id: string };
			"/nodes/[id]/networks": { id: string };
			"/nodes/[id]/storage": { id: string };
			"/nodes/[id]/vms": { id: string };
			"/operations": Record<string, never>;
			"/quotas": Record<string, never>;
			"/settings": Record<string, never>;
			"/storage": Record<string, never>;
			"/templates": Record<string, never>;
			"/vms": { id?: string };
			"/vms/[id]": { id: string }
		};
		Pathname(): "/" | "/backup-jobs" | "/events" | "/images" | "/install" | "/login" | "/metrics" | "/networks" | `/networks/${string}` & {} | "/nodes" | `/nodes/${string}` & {} | `/nodes/${string}/images` & {} | `/nodes/${string}/networks` & {} | `/nodes/${string}/storage` & {} | `/nodes/${string}/vms` & {} | "/operations" | "/quotas" | "/settings" | "/storage" | "/templates" | "/vms" | `/vms/${string}` & {};
		ResolvedPathname(): `${"" | `/${string}`}${ReturnType<AppTypes['Pathname']>}`;
		Asset(): string & {};
	}
}