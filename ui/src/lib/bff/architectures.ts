import { bffFetch, BFFError } from './client';
import { BFFEndpoints } from './endpoints';

/**
 * Phase 0 contract for the Architecture Designer.
 *
 * The shapes here mirror what the backend (separate agent) will implement.
 * Field names match the locked plan: see
 * `docs/plans/2026-06-13-architecture-designer-roadmap.md` and
 * `docs/specs/component/architecture-designer-ui.md`.
 *
 * Optimistic concurrency: every update/archive request carries
 * `expected_version`. If the server's stored version does not match, it
 * returns HTTP 409 and we rethrow as {@link StaleVersionError}.
 */

export type ArchitectureEnvironment = 'development' | 'staging' | 'production';

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

export interface ArchitectureSummary {
	id: string;
	name: string;
	description: string;
	environment: ArchitectureEnvironment;
	status: ArchitectureStatus;
	version_number: number;
	created_at: string;
	updated_at: string;
}

export interface Architecture extends ArchitectureSummary {
	// In Phase 0 the detail object is just the summary plus an empty
	// `spec`/`graph` placeholder. Phase 1 (YAML) and Phase 2 (Svelte Flow)
	// will extend this contract.
}

export interface ListArchitecturesRequest {
	page?: number;
	page_size?: number;
}

export interface ListArchitecturesResponse {
	items: ArchitectureSummary[];
	page: { page: number; page_size: number; total_items: number };
}

export interface GetArchitectureRequest {
	id: string;
}

export interface GetArchitectureResponse {
	architecture: Architecture;
}

export interface CreateArchitectureRequest {
	name: string;
	description?: string;
	environment: ArchitectureEnvironment;
}

export interface CreateArchitectureResponse {
	architecture: Architecture;
}

export interface UpdateArchitecturePatch {
	name?: string;
	description?: string;
	environment?: ArchitectureEnvironment;
}

export interface UpdateArchitectureRequest {
	id: string;
	expected_version: number;
	patch: UpdateArchitecturePatch;
}

export interface UpdateArchitectureResponse {
	architecture: Architecture;
}

export interface ArchiveArchitectureRequest {
	id: string;
	expected_version: number;
}

export interface ArchiveArchitectureResponse {
	architecture: Architecture;
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
