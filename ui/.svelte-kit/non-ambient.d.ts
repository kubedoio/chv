
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
		RouteId(): "/" | "/images" | "/install" | "/login" | "/networks" | "/operations" | "/settings" | "/storage" | "/vms" | "/vms/[id]";
		RouteParams(): {
			"/vms/[id]": { id: string }
		};
		LayoutParams(): {
			"/": { id?: string };
			"/images": Record<string, never>;
			"/install": Record<string, never>;
			"/login": Record<string, never>;
			"/networks": Record<string, never>;
			"/operations": Record<string, never>;
			"/settings": Record<string, never>;
			"/storage": Record<string, never>;
			"/vms": { id?: string };
			"/vms/[id]": { id: string }
		};
		Pathname(): "/" | "/images" | "/install" | "/login" | "/networks" | "/operations" | "/settings" | "/storage" | "/vms" | `/vms/${string}` & {};
		ResolvedPathname(): `${"" | `/${string}`}${ReturnType<AppTypes['Pathname']>}`;
		Asset(): string & {};
	}
}