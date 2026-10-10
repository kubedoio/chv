/**
 * Guest monitoring agent administration (ADR-026, campaign #602 G3).
 *
 * These calls hit the authenticated BFF only. The shapes mirror the
 * operator surface in
 * `chv-controlplane-service/src/api/agent_admin.rs`:
 *
 * - The enrollment claim token is returned by the issuance call
 *   exactly once and is never persisted client-side or re-fetchable —
 *   the UI renders it in a modal and forgets it.
 * - Agent state uses the security contract's wire vocabulary,
 *   including the documented `identity_conflict` extension for the
 *   cloned-image signal.
 * - `guest_ingestion_disabled` means the deployment never enabled
 *   guest ingestion — it is a typed condition, not an empty list that
 *   would read as "no agents enrolled".
 */

import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

/** Wire state vocabulary (security contract + identity_conflict). */
export type GuestAgentState =
	| 'active'
	| 'renewal_due'
	| 'offline'
	| 'expired'
	| 'revoked'
	| 'identity_conflict'
	| 'unenrolled'
	| 'enrolling';

export interface GuestAgentOs {
	name: string | null;
	version: string | null;
	kernel_release: string | null;
}

export interface GuestAgentInventoryItem {
	/** Null on the synthesized `enrolling` entry (claim issued, not yet redeemed). */
	agent_id: string | null;
	vm_id: string;
	state: GuestAgentState;
	/** Null on the `enrolling` entry — no install exists yet. */
	install_id: string | null;
	/** Null on the `enrolling` entry — no credential exists yet. */
	credential_epoch: number | null;
	credential_expires_at_ms: number | null;
	rotation_pending: boolean;
	identity_conflict: boolean;
	conflict_reason: string | null;
	/** Null on the `enrolling` entry — nothing has enrolled yet. */
	enrolled_at_ms: number | null;
	last_seen_at_ms: number | null;
	last_seen_age_seconds: number | null;
	os: GuestAgentOs;
	/** Present only on the `enrolling` entry: the live claim's expiry. */
	claim_expires_at_ms?: number | null;
	/** Present only on the `enrolling` entry: who issued the claim. */
	claim_issued_by?: string | null;
}

export interface GuestAgentInventoryResponse {
	schema_version: number;
	agents: GuestAgentInventoryItem[];
	generated_at_ms: number;
	truncated: boolean;
}

/** The one-time enrollment claim. Never persisted, never re-shown. */
export interface GuestAgentClaimResponse {
	schema_version: number;
	vm_id: string;
	claim_token: string;
	expires_at_ms: number;
	server_url: string | null;
	ca_fingerprint: string | null;
}

export async function listGuestAgents(
	vmId?: string,
	token?: string
): Promise<GuestAgentInventoryResponse> {
	return bffFetch<GuestAgentInventoryResponse>(BFFEndpoints.monitoringAgents, {
		method: 'POST',
		body: JSON.stringify(vmId ? { vm_id: vmId } : {}),
		token
	});
}

export async function issueGuestAgentClaim(
	vmId: string,
	token?: string
): Promise<GuestAgentClaimResponse> {
	return bffFetch<GuestAgentClaimResponse>(BFFEndpoints.monitoringAgentClaim, {
		method: 'POST',
		body: JSON.stringify({ vm_id: vmId }),
		token
	});
}

export async function revokeGuestAgent(
	agentId: string,
	token?: string
): Promise<{ agent_id: string; state: string }> {
	return bffFetch(BFFEndpoints.monitoringAgentRevoke, {
		method: 'POST',
		body: JSON.stringify({ agent_id: agentId }),
		token
	});
}

export async function forceGuestAgentRotation(
	agentId: string,
	token?: string
): Promise<{ agent_id: string; rotation_pending: boolean }> {
	return bffFetch(BFFEndpoints.monitoringAgentRotate, {
		method: 'POST',
		body: JSON.stringify({ agent_id: agentId }),
		token
	});
}

export async function resetGuestAgentConflict(
	agentId: string,
	token?: string
): Promise<{ agent_id: string; identity_conflict: boolean }> {
	return bffFetch(BFFEndpoints.monitoringAgentReset, {
		method: 'POST',
		body: JSON.stringify({ agent_id: agentId }),
		token
	});
}
