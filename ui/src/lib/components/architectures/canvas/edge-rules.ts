/**
 * Edge validation rules for the Architecture Designer canvas.
 *
 * Source of truth: `docs/specs/architecture-designer/contracts/graph-contract.md`
 * §"Edge validation rules". The matrix below is the MVP subset — only the
 * eight MVP node kinds are honored. The contract lists `instance →
 * backup_policy uses_backup_policy`, but `backup_policy` is not part of the
 * MVP node kinds (only `host network datastore image template instance user
 * role`), so that row is intentionally omitted here. When backup-policy
 * nodes are added (post-MVP), append the row and the corresponding palette
 * entry — both in the same change so the compile-time exhaustiveness check
 * in `palette.test.ts` keeps the two registries in sync.
 *
 * These rules are UX-time guards: they reject the drop and surface a toast
 * before the graph reaches the server. The Rust validator
 * (`chv-architecture-validate`) catches the same class of mistake at
 * validate-time as `INVALID_EDGE`, so the rules here are advisory, not
 * security-relevant.
 */

export type NodeKind =
	| 'host'
	| 'network'
	| 'datastore'
	| 'image'
	| 'template'
	| 'instance'
	| 'user'
	| 'role';

export const MVP_NODE_KINDS: ReadonlyArray<NodeKind> = [
	'host',
	'network',
	'datastore',
	'image',
	'template',
	'instance',
	'user',
	'role'
];

export type EdgeType =
	| 'placed_on'
	| 'attached_to_network'
	| 'uses_datastore'
	| 'uses_image'
	| 'uses_template'
	| 'has_role'
	| 'uses_backup_policy';

export interface EdgeRule {
	readonly source: NodeKind;
	readonly target: NodeKind;
	readonly edgeType: EdgeType;
}

/**
 * Allowed (source, target, edgeType) triples. Anything not in this list is
 * rejected by `isEdgeAllowed`.
 *
 * Rows mirror graph-contract.md exactly, MVP subset only:
 *   - instance → host          : placed_on
 *   - instance → network       : attached_to_network
 *   - instance → datastore     : uses_datastore
 *   - template → image         : uses_image
 *   - instance → template      : uses_template
 *   - user     → role          : has_role
 *
 * NOT included (post-MVP, the target kind is not in NodeKind):
 *   - instance → backup_policy : uses_backup_policy
 */
export const EDGE_RULES: ReadonlyArray<EdgeRule> = [
	{ source: 'instance', target: 'host', edgeType: 'placed_on' },
	{ source: 'instance', target: 'network', edgeType: 'attached_to_network' },
	{ source: 'instance', target: 'datastore', edgeType: 'uses_datastore' },
	{ source: 'template', target: 'image', edgeType: 'uses_image' },
	{ source: 'instance', target: 'template', edgeType: 'uses_template' },
	{ source: 'user', target: 'role', edgeType: 'has_role' }
];

/**
 * MVP-only edge types — those whose source AND target kinds are both in
 * `MVP_NODE_KINDS`. `uses_backup_policy` is not here because its target
 * (`backup_policy`) is post-MVP.
 */
export const MVP_EDGE_TYPES: ReadonlyArray<EdgeType> = [
	'placed_on',
	'attached_to_network',
	'uses_datastore',
	'uses_image',
	'uses_template',
	'has_role'
];

/**
 * True iff the (source, target, edgeType) triple appears in `EDGE_RULES`.
 * The MVP rules are deduped (no two rows share the same triple), so a
 * simple linear scan is correct and fast.
 */
export function isEdgeAllowed(source: NodeKind, target: NodeKind, edgeType: EdgeType): boolean {
	for (const rule of EDGE_RULES) {
		if (rule.source === source && rule.target === target && rule.edgeType === edgeType) {
			return true;
		}
	}
	return false;
}

/**
 * Pick a default edge type for a (source, target) drag. Returns the FIRST
 * matching row in `EDGE_RULES` declaration order, or `null` if no rule
 * matches. With the current MVP rules, every (source, target) combination
 * has at most one allowed edge type — but document the first-match
 * behaviour anyway so callers can rely on it if more rules are added later.
 */
export function inferEdgeType(source: NodeKind, target: NodeKind): EdgeType | null {
	for (const rule of EDGE_RULES) {
		if (rule.source === source && rule.target === target) {
			return rule.edgeType;
		}
	}
	return null;
}
