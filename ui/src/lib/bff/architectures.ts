import { bffFetch, BFFError } from './client';
import { BFFEndpoints } from './endpoints';

/**
 * Phase 0 contract for the Architecture Designer.
 *
 * The shapes here mirror the wire format emitted by the Rust BFF
 * (`crates/chv-webui-bff/src/handlers/architectures.rs`). Field names match
 * the locked plan: see
 * `docs/plans/2026-06-13-architecture-designer-roadmap.md` and
 * `docs/specs/component/architecture-designer-ui.md`.
 *
 * Important wire-shape notes (see PR review B1-B4):
 *   - List response is `{ architectures: [...] }` — there is NO `items` /
 *     `page` wrapper.
 *   - Update is FLAT: `{ id, expected_version, display_name?, description?, ... }` —
 *     there is NO `patch` wrapper. The wire field is `display_name`, not `name`.
 *   - Archive requires `expected_version` and returns the archived row inside
 *     `{ architecture }`.
 *   - Several fields are nullable on the wire (description, environment,
 *     display_name, owner_user_id, archived_at, last_*_status). Consumers
 *     must use `?? ''` or explicit null-checks.
 *
 * Optimistic concurrency: every update/archive request carries
 * `expected_version`. If the server's stored version does not match, it
 * returns HTTP 409 and we rethrow as {@link StaleVersionError}.
 */

/**
 * Display-only environment hints. The wire accepts any free-form string for
 * `environment`; this union exists so dropdowns and badges can switch on the
 * usual three values without losing the ability to show an unknown one.
 */
export type ArchitectureEnvironment = 'development' | 'staging' | 'production';

export const KNOWN_ARCHITECTURE_ENVIRONMENTS: readonly ArchitectureEnvironment[] = [
	'development',
	'staging',
	'production'
];

export type ArchitectureStatus =
	| 'draft'
	| 'valid'
	| 'invalid'
	| 'planned'
	| 'applying'
	| 'applied'
	| 'drifted'
	| 'failed'
	| 'archived';

export type ArchitectureCheckStatus = 'unknown' | 'passed' | 'failed';

export interface ArchitectureSummary {
	id: string;
	name: string;
	display_name: string | null;
	description: string | null;
	environment: string | null;
	status: ArchitectureStatus;
	owner_user_id: string | null;
	last_validation_status: ArchitectureCheckStatus | null;
	last_fleet_check_status: ArchitectureCheckStatus | null;
	version_number: number;
	created_at: string;
	updated_at: string;
	archived_at: string | null;
}

/**
 * Phase 0 detail object equals the summary — no extra columns yet. Phase 1
 * (YAML) and Phase 2 (Svelte Flow) will extend ArchitectureDetail's payload,
 * not Architecture itself.
 */
export type Architecture = ArchitectureSummary;

/**
 * Full get-response payload, including the optional design graph and YAML
 * blobs that the BFF now returns. Both blobs are nullable for newly created
 * architectures.
 */
export interface ArchitectureDetail {
	architecture: ArchitectureSummary;
	design_graph_json: string | null;
	latest_yaml: string | null;
}

export interface ListArchitecturesRequest {
	include_archived?: boolean;
}

export interface ListArchitecturesResponse {
	architectures: ArchitectureSummary[];
}

export interface GetArchitectureRequest {
	id: string;
}

export type GetArchitectureResponse = ArchitectureDetail;

export interface CreateArchitectureRequest {
	name: string;
	description?: string | null;
	environment?: string | null;
	display_name?: string | null;
	design_graph_json?: string | null;
	latest_yaml?: string | null;
}

export interface CreateArchitectureResponse {
	architecture: ArchitectureSummary;
}

/**
 * FLAT update request — fields live at the top level alongside `id` and
 * `expected_version`. There is no `patch` wrapper. All optional fields accept
 * `null` to explicitly clear a value (see backend handler).
 */
export interface UpdateArchitectureRequest {
	id: string;
	expected_version: number;
	display_name?: string | null;
	description?: string | null;
	environment?: string | null;
	design_graph_json?: string | null;
	latest_yaml?: string | null;
	latest_version_id?: string | null;
}

export interface UpdateArchitectureResponse {
	architecture: ArchitectureSummary;
}

export interface ArchiveArchitectureRequest {
	id: string;
	expected_version: number;
}

export interface ArchiveArchitectureResponse {
	architecture: ArchitectureSummary;
}

/**
 * Thrown by `update`/`archive` when the BFF reports a 409 Conflict, indicating
 * the client's `expected_version` is behind the stored version. Callers should
 * surface a "stale version" banner with a Reload button. See ADR-001-Designer
 * and the Phase 0 plan, Q3.
 */
export class StaleVersionError extends Error {
	public readonly status = 409;
	public readonly code: string;
	public readonly architectureId: string;
	public readonly expectedVersion: number;

	constructor(architectureId: string, expectedVersion: number, message?: string, code = 'STALE_VERSION') {
		super(message ?? `Architecture ${architectureId} was modified by someone else (expected v${expectedVersion}).`);
		this.name = 'StaleVersionError';
		this.code = code;
		this.architectureId = architectureId;
		this.expectedVersion = expectedVersion;
	}
}

function rethrowAsStaleVersion(err: unknown, architectureId: string, expectedVersion: number): never {
	if (err instanceof BFFError && err.status === 409) {
		throw new StaleVersionError(architectureId, expectedVersion, err.message, err.code);
	}
	throw err;
}

export async function listArchitectures(
	req: ListArchitecturesRequest = {},
	token?: string
): Promise<ListArchitecturesResponse> {
	return bffFetch<ListArchitecturesResponse>(BFFEndpoints.listArchitectures, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function getArchitecture(
	req: GetArchitectureRequest,
	token?: string
): Promise<GetArchitectureResponse> {
	return bffFetch<GetArchitectureResponse>(BFFEndpoints.getArchitecture, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function createArchitecture(
	req: CreateArchitectureRequest,
	token?: string
): Promise<CreateArchitectureResponse> {
	return bffFetch<CreateArchitectureResponse>(BFFEndpoints.createArchitecture, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function updateArchitecture(
	req: UpdateArchitectureRequest,
	token?: string
): Promise<UpdateArchitectureResponse> {
	try {
		return await bffFetch<UpdateArchitectureResponse>(BFFEndpoints.updateArchitecture, {
			method: 'POST',
			body: JSON.stringify(req),
			token
		});
	} catch (err) {
		rethrowAsStaleVersion(err, req.id, req.expected_version);
	}
}

export async function archiveArchitecture(
	req: ArchiveArchitectureRequest,
	token?: string
): Promise<ArchiveArchitectureResponse> {
	try {
		return await bffFetch<ArchiveArchitectureResponse>(BFFEndpoints.archiveArchitecture, {
			method: 'POST',
			body: JSON.stringify(req),
			token
		});
	} catch (err) {
		rethrowAsStaleVersion(err, req.id, req.expected_version);
	}
}

// ─── Phase 1: Validation + YAML wire types ────────────────────────────────
//
// Source of truth: docs/specs/architecture-designer/contracts/validation-plan-contract.md
//
// The BFF computes a structured ValidationResult — a status pill, a summary
// of counts by severity, and an ordered list of findings. Findings are
// stable across calls for the same input (so the UI can sort by severity →
// code → path without resorting to server-side ordering).

export type ValidationSeverity = 'error' | 'warning' | 'info';
export type ValidationStatus = 'valid' | 'warning' | 'invalid';

export interface Finding {
	severity: ValidationSeverity;
	/** Stable string code from the registry (e.g. SCHEMA_INVALID, DUPLICATE_NAME). */
	code: string;
	message: string;
	/** JSON path within the topology, e.g. "instances[0].disks[1].datastore". */
	path: string;
	/** Human-friendly resource ref, e.g. "instances/app-01"; null when not bound to a resource. */
	resource_ref: string | null;
	/** Apply pipelines must refuse to proceed when blocking is true. */
	blocking: boolean;
	suggestion: string | null;
}

export interface ValidationSummary {
	errors: number;
	warnings: number;
	info: number;
}

export interface ValidationResult {
	status: ValidationStatus;
	summary: ValidationSummary;
	findings: Finding[];
}

export interface ValidateArchitectureRequest {
	id: string;
}

export interface ValidateYamlRequest {
	yaml: string;
}

export interface GenerateYamlRequest {
	id: string;
}

export interface GenerateYamlResponse {
	yaml: string;
}

export interface ImportYamlRequest {
	id: string;
	yaml: string;
}

export interface ImportYamlResponse {
	result: ValidationResult;
}

/**
 * Run server-side validation against a saved topology.
 *
 * The server reads the latest persisted graph + YAML and recomputes findings;
 * it also persists `last_validation_status` so the dashboard pill stays in
 * sync. The findings list itself is NOT persisted — re-validate to see them
 * again after a page reload.
 */
export async function validateArchitecture(
	req: ValidateArchitectureRequest,
	token?: string
): Promise<ValidationResult> {
	return bffFetch<ValidationResult>(BFFEndpoints.validateArchitecture, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

/**
 * Ad-hoc validation of a YAML string with no persistent target.
 *
 * Used by the Import dialog so operators can verify YAML before committing
 * it to a topology. Pure read-side — does not mutate any architecture row.
 */
export async function validateYaml(
	req: ValidateYamlRequest,
	token?: string
): Promise<ValidationResult> {
	return bffFetch<ValidationResult>(BFFEndpoints.validateYaml, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

/**
 * Generate the canonical YAML serialisation of a saved topology.
 *
 * May 422 with `code === 'GRAPH_EMPTY'` when the topology has no graph and
 * no `latest_yaml` (Phase 2 supplies the canvas → graph mapping; Phase 1
 * just lays the wire). Callers should surface this as a helpful empty state
 * rather than an error toast.
 */
export async function generateYaml(
	req: GenerateYamlRequest,
	token?: string
): Promise<GenerateYamlResponse> {
	return bffFetch<GenerateYamlResponse>(BFFEndpoints.generateYaml, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

/**
 * Import a YAML blob into the topology, validate it, and persist the result.
 *
 * The server stores `latest_yaml` and updates `last_validation_status` on
 * the architecture row, so the dashboard pill reflects the import outcome.
 * Findings are returned inline; they are NOT persisted (see {@link validateArchitecture}).
 */
export async function importYaml(
	req: ImportYamlRequest,
	token?: string
): Promise<ImportYamlResponse> {
	return bffFetch<ImportYamlResponse>(BFFEndpoints.importYaml, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

// ─── Phase 3: Fleet consistency check wire types ─────────────────────────
//
// Layer 2 validates the topology against the live fleet inventory captured
// at request time. The BFF persists `last_fleet_check_status` on the
// architecture row and stores the inventory snapshot keyed by
// `inventory_snapshot_id` for later forensic replay. Findings reuse the
// same `Finding` shape as Phase 1 validation; the per-row `status` here is
// the fleet-check verdict, not the schema verdict.

export interface FleetCheckResult {
	status: ValidationStatus;
	/** Stable id of the inventory snapshot the server captured for this run. */
	inventory_snapshot_id: string;
	/** RFC3339 timestamp of when the snapshot was taken (server clock). */
	checked_at: string;
	findings: Finding[];
}

export interface CheckFleetRequest {
	id: string;
}

/**
 * Run Layer 2 fleet-consistency checks against a saved topology.
 *
 * The server captures a fresh inventory snapshot, evaluates fleet checks
 * (host capacity, network availability, datastore capacity, image presence,
 * backup-target reachability), and persists `last_fleet_check_status`.
 * Findings are returned inline; the snapshot itself is keyed by
 * `inventory_snapshot_id` so a future replay flow can re-run checks against
 * the captured fleet without re-snapshotting.
 */
export async function checkFleet(
	req: CheckFleetRequest,
	token?: string
): Promise<FleetCheckResult> {
	return bffFetch<FleetCheckResult>(BFFEndpoints.architecturesCheckFleet, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}
