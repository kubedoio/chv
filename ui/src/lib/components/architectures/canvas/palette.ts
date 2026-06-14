import type { NodeKind } from './edge-rules';
import { MVP_NODE_KINDS } from './edge-rules';

/**
 * Drag-source palette registry.
 *
 * Adding a new resource kind is a two-place change by design:
 *   1. Add the kind to `NodeKind` and (if it's MVP) `MVP_NODE_KINDS` in
 *      `edge-rules.ts`.
 *   2. Add a matching `PaletteEntry` here.
 *
 * `palette.test.ts` enforces this with a compile-time exhaustiveness check —
 * if step (2) is forgotten, TypeScript will flag the missing kind.
 *
 * `icon` names are lucide icon identifiers (kebab-case). Components consume
 * them via `lucide-svelte`'s dynamic icon import.
 */

export interface PaletteEntry {
	readonly kind: NodeKind;
	readonly label: string;
	readonly description: string;
	/** lucide-svelte icon name, e.g. `server`, `network`, `database`. */
	readonly icon: string;
}

export const PALETTE: ReadonlyArray<PaletteEntry> = [
	{
		kind: 'host',
		label: 'Host',
		description: 'Physical or virtual host that runs Cloud Hypervisor.',
		icon: 'server'
	},
	{
		kind: 'network',
		label: 'Network',
		description: 'Virtual network that instances attach to.',
		icon: 'network'
	},
	{
		kind: 'datastore',
		label: 'Datastore',
		description: 'Storage backend for instance disks.',
		icon: 'database'
	},
	{
		kind: 'image',
		label: 'Image',
		description: 'Boot image used by templates.',
		icon: 'disc'
	},
	{
		kind: 'template',
		label: 'Template',
		description: 'Reusable instance specification (CPU, memory, image).',
		icon: 'layout-template'
	},
	{
		kind: 'instance',
		label: 'Instance',
		description: 'Running VM placed on a host.',
		icon: 'box'
	},
	{
		kind: 'user',
		label: 'User',
		description: 'Identity that can be assigned roles.',
		icon: 'user'
	},
	{
		kind: 'role',
		label: 'Role',
		description: 'Permission grant assignable to users.',
		icon: 'shield'
	}
];

// Re-export so palette consumers don't need a second import for the kind list.
export { MVP_NODE_KINDS };
