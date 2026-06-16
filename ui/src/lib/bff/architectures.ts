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

// ─── Phase 4: Plan generation wire types ─────────────────────────────────
//
// Source of truth: docs/specs/architecture-designer/contracts/validation-plan-contract.md
// and docs/specs/architecture-designer/contracts/api-contract.md.
//
// The BFF computes a structured PlanResult — a unique plan_id, a status (one
// of the documented plan states), a per-action summary, the ordered list of
// PlanChange rows, optional warnings, and a TTL window (`expires_at`). Plans
// are persisted server-side with a 15 minute TTL; the UI surfaces a live
// countdown so operators don't act on a stale plan.

export type PlanAction = 'create' | 'update' | 'delete' | 'replace' | 'no_op';

export type PlanRisk = 'low' | 'medium' | 'high' | 'destructive';

export type PlanMode = 'apply' | 'destroy';

export type PlanStatus =
	| 'draft'
	| 'failed_validation'
	| 'requires_confirmation'
	| 'ready_to_apply'
	| 'applying'
	| 'applied'
	| 'failed'
	| 'expired'
	| 'discarded';

export interface PlanChange {
	action: PlanAction;
	resource_type: string;
	resource_name: string;
	resource_ref: string;
	description: string;
	risk: PlanRisk;
	requires_confirmation: boolean;
}

export interface PlanSummary {
	create: number;
	update: number;
	delete: number;
	replace: number;
	no_op: number;
	warnings: number;
}

export interface PlanResult {
	plan_id: string;
	architecture_id: string;
	/** Numeric topology version the plan was generated against. Apply must
	 * pin to this version so a stale operator doesn't blast a divergent state. */
	architecture_version: number;
	/** Stable id of the `architecture_versions` row referenced by the plan. */
	architecture_version_id: string;
	status: PlanStatus;
	mode: PlanMode;
	summary: PlanSummary;
	changes: PlanChange[];
	warnings: string[];
	/** ISO 8601 — UI compares against `Date.now()` to surface the expired state. */
	expires_at: string;
	created_at: string;
}

export interface PlanArchitectureRequest {
	id: string;
	allow_warnings?: boolean;
	refresh_inventory?: boolean;
}

export interface DestroyPlanRequest {
	id: string;
}

export interface DiscardPlanRequest {
	plan_id: string;
}

export interface DiscardPlanResponse {
	status: 'discarded';
}

/**
 * Generate an apply-mode plan for a saved topology.
 *
 * The server validates the topology, captures (or reuses) an inventory
 * snapshot, computes the desired-vs-actual diff, persists the resulting plan
 * with a 15 minute TTL, and returns the structured PlanResult. The plan row
 * also stamps `last_plan_status` on the architecture so the dashboard pill
 * stays in sync after the call goes through `mutateWithRefresh`.
 */
export async function plan(
	req: PlanArchitectureRequest,
	token?: string
): Promise<PlanResult> {
	return bffFetch<PlanResult>(BFFEndpoints.architecturesPlan, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

/**
 * Generate a destroy-mode plan for a saved topology.
 *
 * Same TTL semantics as {@link plan}. Every change in a destroy plan carries
 * `requires_confirmation: true`; the UI gates the apply button behind a
 * typed-name confirmation field.
 */
export async function destroyPlan(
	req: DestroyPlanRequest,
	token?: string
): Promise<PlanResult> {
	return bffFetch<PlanResult>(BFFEndpoints.architecturesDestroyPlan, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

/**
 * Explicitly discard a plan before its TTL expires.
 *
 * Idempotent on the server side — discarding an already-discarded plan
 * returns 200. Goes through `mutateWithRefresh` because the architecture's
 * `last_plan_status` is updated.
 */
export async function discardPlan(
	req: DiscardPlanRequest,
	token?: string
): Promise<DiscardPlanResponse> {
	return bffFetch<DiscardPlanResponse>(BFFEndpoints.architecturesDiscardPlan, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

// ─── Phase 5: Apply / runs wire types ────────────────────────────────────
//
// Source of truth: docs/plans/2026-06-13-architecture-designer-implementation-plan.md
// §3 Phase 5 and crates/chv-webui-bff/src/handlers/architectures.rs.
//
// Wire endpoints exposed by the BFF in this phase:
//   - POST /v1/architectures/apply           → ApplyRunResult
//   - POST /v1/architectures/destroy         → ApplyRunResult
//   - POST /v1/architectures/runs/list       → { runs: ApplyRunDetail[] }
//
// Endpoint coverage:
//   The BFF ships POST /v1/architectures/apply, /destroy, and /runs/list.
//   `listApplyRuns` calls `runs/list` directly — no fallback. There is still
//   no per-run GET endpoint (`/runs/get`); for MVP the single-run page lists
//   all runs for the architecture and plucks by id. This is correct under the
//   current data model (runs are always architecture-scoped and the apply
//   response carries `architecture_id`). When the BFF adds `/runs/get`,
//   swap `getApplyRun`'s body — the public signature stays stable.

/**
 * Lifecycle states for an `architecture_apply_runs` row. Mirrors the Rust
 * `RunStatus` enum so the UI can switch on a single discriminant. Terminal
 * states are succeeded / partially_failed / failed / cancelled — the runs
 * store stops polling once a run lands in any of them.
 */
export type RunStatus =
	| 'queued'
	| 'running'
	| 'succeeded'
	| 'partially_failed'
	| 'failed'
	| 'cancelled';

/**
 * Response shape for the apply/destroy endpoints. The BFF inserts the run
 * synchronously and enqueues the first Operation; the orchestrator then
 * advances the run through Running → terminal asynchronously. `task_id` is
 * the first Operation's id and lets clients stream via /v1/operations/{id}.
 */
export interface ApplyRunResult {
	run_id: string;
	task_id: string | null;
	status: RunStatus;
	started_at: string | null;
	architecture_id: string;
	architecture_version_id: string;
	plan_id: string;
}

/**
 * Full row for an apply run. `result_json` is a stringified JSON payload
 * (set by the orchestrator on terminal transitions) summarising per-op
 * outcomes; the run page parses it lazily. `error_message` is populated
 * for `failed`/`partially_failed` runs.
 */
export interface ApplyRunDetail {
	id: string;
	architecture_id: string;
	architecture_version_id: string;
	plan_id: string | null;
	task_id: string | null;
	status: RunStatus;
	started_at: string | null;
	finished_at: string | null;
	requested_by: string | null;
	result_json: string | null;
	error_message: string | null;
	created_at: string;
	updated_at: string;
}

export interface ConfirmationToken {
	typed_name?: string;
}

export interface ApplyArchitectureRequest {
	id: string;
	plan_id: string;
	confirmation: ConfirmationToken;
	acknowledged_warnings: boolean;
}

export interface ListApplyRunsRequest {
	/**
	 * Architecture id. Wire field is `id` (not `architecture_id`) for parity
	 * with every other architecture-scoped DTO in this module — see review
	 * note M1 in the Phase-6 fix-up.
	 */
	id: string;
}

export interface ListApplyRunsResponse {
	runs: ApplyRunDetail[];
}

/**
 * Apply a plan: queues an ApplyRun and returns its id + the first
 * Operation's id (`task_id`). Caller is expected to navigate to
 * `/architectures/{id}/runs/{run_id}` and poll for status. The BFF rejects
 * with 400 `MISSING_CONFIRMATION` when typed-name is required but absent
 * or wrong, 400 `WARNINGS_NOT_ACKNOWLEDGED` when warnings exist but
 * `acknowledged_warnings` is false, 403 `INSUFFICIENT_PERMISSIONS` for
 * non-admin against production, 409 `PLAN_EXPIRED` / `PLAN_NOT_APPLICABLE`
 * for stale plan rows.
 */
export async function apply(
	id: string,
	plan_id: string,
	confirmation: ConfirmationToken,
	acknowledged_warnings: boolean,
	token?: string
): Promise<ApplyRunResult> {
	const req: ApplyArchitectureRequest = {
		id,
		plan_id,
		confirmation,
		acknowledged_warnings
	};
	return bffFetch<ApplyRunResult>(BFFEndpoints.architecturesApply, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

/**
 * Apply a destroy-mode plan. Same wire contract as {@link apply} but the
 * BFF gates additionally on the destroy plan's `requires_confirmation`
 * (typed-name is mandatory, regardless of risk).
 */
export async function destroy(
	id: string,
	plan_id: string,
	confirmation: ConfirmationToken,
	acknowledged_warnings: boolean,
	token?: string
): Promise<ApplyRunResult> {
	const req: ApplyArchitectureRequest = {
		id,
		plan_id,
		confirmation,
		acknowledged_warnings
	};
	return bffFetch<ApplyRunResult>(BFFEndpoints.architecturesDestroy, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

/**
 * List apply runs for a single architecture, newest first.
 *
 * Wire body is `{ id }` (matches the `{ id }` convention the rest of the
 * architecture endpoints use); the response is `{ runs: [...] }`. Repository
 * ordering already returns newest-first, so callers can render straight to a
 * table without re-sorting.
 */
export async function listApplyRuns(
	architecture_id: string,
	token?: string
): Promise<ApplyRunDetail[]> {
	const req: ListApplyRunsRequest = { id: architecture_id };
	const res = await bffFetch<ListApplyRunsResponse>(BFFEndpoints.architecturesRunsList, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
	return res.runs ?? [];
}

/**
 * Fetch a single apply run by id.
 *
 * Implementation note: the BFF does not (yet) expose `/v1/architectures/runs/get`.
 * We pull the architecture-scoped list and pluck by id. This is correct
 * under the current data model — runs are always associated with an
 * architecture, and the apply response carries `architecture_id`. When the
 * BFF adds a per-run GET, swap this body for a single fetch; the public
 * signature stays stable.
 *
 * Throws BFFError with `code: 'NOT_FOUND'` (status 404) when the run is
 * not present in the list.
 */
export async function getApplyRun(
	architecture_id: string,
	run_id: string,
	token?: string
): Promise<ApplyRunDetail> {
	const runs = await listApplyRuns(architecture_id, token);
	const run = runs.find((r) => r.id === run_id);
	if (!run) {
		throw new BFFError(`Run ${run_id} not found for architecture ${architecture_id}`, 404, 'NOT_FOUND');
	}
	return run;
}

/**
 * Discriminator for the terminal transition. The store uses this to stop
 * its polling loop. `cancelled` is included because Phase 5 reserves it
 * for future use (operator cancel).
 */
export const TERMINAL_RUN_STATUSES: readonly RunStatus[] = [
	'succeeded',
	'partially_failed',
	'failed',
	'cancelled'
];

export function isTerminalRunStatus(status: RunStatus): boolean {
	return TERMINAL_RUN_STATUSES.includes(status);
}

// ─── Phase 6: Drift detection wire types ─────────────────────────────────
//
// Source of truth: docs/plans/2026-06-13-architecture-designer-implementation-plan.md
// §3 Phase 6 and crates/chv-architecture-reconcile/src/drift.rs.
//
// Drift = the diff between the architecture model (baseline) and the live
// fleet snapshot, computed pure-data. The BFF endpoint caches results for
// 5 minutes via `state.clock`; pass `force_refresh: true` to bypass.
//
// Wire response fields match the BFF (see handlers/architectures.rs):
//   drift_report_id, status, findings, summary, baseline_version_id,
//   computed_at, cache_hit, error_message.

/**
 * Status pill values for a drift report. `unknown` is the initial state when
 * a topology has never been checked; `check_failed` is emitted when snapshot
 * capture or compute itself errored (the report row is persisted with an
 * `error_message` so the UI can banner the cause).
 */
export type DriftStatus = 'unknown' | 'no_drift' | 'drifted' | 'check_failed';

/**
 * Discriminated union over the 7 drift finding codes shipped by the Rust
 * `compute_drift` function. Each variant carries enough data for the row
 * renderer to label the change without an extra round-trip. `path` is a
 * JSON-pointer-style string within the topology (e.g. `instances[0].cpu_cores`).
 * `resource_ref` is the human-friendly handle (e.g. `instances/app-01`).
 *
 * Forward-compat: when Phase 7 adds new codes, callers should fall through
 * to a generic row rather than crashing on an unknown discriminant.
 */
export type DriftFinding =
	| {
			code: 'DRIFT_MISSING_RESOURCE';
			path: string;
			resource_ref: string;
			message: string;
	  }
	| {
			code: 'DRIFT_UNEXPECTED_RESOURCE';
			path: string;
			resource_ref: string;
			message: string;
	  }
	| {
			code: 'DRIFT_FIELD_CHANGED';
			path: string;
			resource_ref: string;
			field: string;
			expected: string;
			actual: string;
			message: string;
	  }
	| {
			code: 'DRIFT_CAPACITY_CHANGED';
			path: string;
			resource_ref: string;
			field: string;
			expected: number;
			actual: number;
			message: string;
	  }
	| {
			code: 'DRIFT_NETWORK_CHANGED';
			path: string;
			resource_ref: string;
			field: string;
			expected: string | null;
			actual: string | null;
			message: string;
	  }
	| {
			code: 'DRIFT_PERMISSION_CHANGED';
			path: string;
			resource_ref: string;
			message: string;
	  }
	| {
			code: 'DRIFT_ATTACHMENT_CHANGED';
			path: string;
			resource_ref: string;
			message: string;
	  };

/**
 * The seven stable codes — exported so the panel can iterate them when
 * rendering empty per-type chips. Keep aligned with the Rust enum.
 */
export const DRIFT_FINDING_CODES = [
	'DRIFT_MISSING_RESOURCE',
	'DRIFT_UNEXPECTED_RESOURCE',
	'DRIFT_FIELD_CHANGED',
	'DRIFT_CAPACITY_CHANGED',
	'DRIFT_NETWORK_CHANGED',
	'DRIFT_PERMISSION_CHANGED',
	'DRIFT_ATTACHMENT_CHANGED'
] as const;

export type DriftFindingCode = (typeof DRIFT_FINDING_CODES)[number];

export interface DriftSummary {
	total: number;
	/**
	 * Server emits one entry per code with a non-zero count. The UI
	 * iterates `DRIFT_FINDING_CODES` and reads `by_type[code] ?? 0` so
	 * a zero-count code shows a 0 chip rather than disappearing.
	 */
	by_type: Record<string, number>;
}

export interface DriftReport {
	/** Stable id of the persisted drift_report row, or null when status===`unknown`. */
	drift_report_id: string | null;
	status: DriftStatus;
	findings: DriftFinding[];
	summary: DriftSummary;
	/** Architecture version id the diff was computed against. */
	baseline_version_id: string;
	/** RFC3339 server timestamp; UI renders relative ("5s ago"). */
	computed_at: string;
	/** True when the BFF returned a cached row instead of recomputing. */
	cache_hit: boolean;
	/** Populated only on `check_failed`. */
	error_message: string | null;
}

export interface GetDriftRequest {
	id: string;
	force_refresh?: boolean;
}

/**
 * Fetch (or recompute) the drift report for an architecture.
 *
 * Pass `force_refresh: true` to bypass the BFF's 5-minute cache and trigger a
 * fresh snapshot capture + recompute. The optional `fetch_` arg is accepted
 * for parity with SvelteKit `load()` callsites; the underlying `bffFetch`
 * uses the global `fetch` so the arg is currently ignored. The optional
 * `signal` arg is forwarded to `bffFetch` so dashboard fan-out callers can
 * abort in-flight requests when they are superseded by a newer effect.
 * Wire-format fields match the BFF response: `drift_report_id`, `status`,
 * `findings`, `summary`, `baseline_version_id`, `computed_at`, `cache_hit`,
 * `error_message`.
 *
 * Throws BFFError when the BFF fails to respond; never throws on
 * `status === 'check_failed'` — that comes back as a normal 200 with the
 * failure status carried in the body.
 */
export async function getArchitectureDrift(
	id: string,
	force_refresh: boolean,
	token?: string,
	signal?: AbortSignal
): Promise<DriftReport> {
	const req: GetDriftRequest = { id, force_refresh };
	return bffFetch<DriftReport>(BFFEndpoints.architecturesDrift, {
		method: 'POST',
		body: JSON.stringify(req),
		token,
		signal
	});
}

/**
 * Discriminator helper for the dashboard sidebar. The badge renders only
 * when the architecture is in a state where drift is meaningful (i.e. it
 * has a baseline). Drift on a `draft` topology is always `unknown` and we
 * suppress the badge to keep the dashboard quiet.
 */
export function isDriftBadgeVisible(status: DriftStatus): boolean {
	return status === 'drifted' || status === 'check_failed' || status === 'no_drift';
}
