/**
 * Presentation helpers for the guest monitoring agent card: state
 * vocabulary mapping to labels, tones, and descriptions. Pure
 * functions — tested in `guestAgent.test.ts` — so the card component
 * stays under the size budget and the vocabulary stays single-sourced.
 */

import type { GuestAgentState } from '#lib/bff/monitoringAgents.ts';

export interface GuestAgentStateView {
	label: string;
	/** Tone token shared with the shell's status vocabulary. */
	tone: 'success' | 'warning' | 'danger' | 'muted' | 'info';
	description: string;
}

const STATE_VIEWS: Record<GuestAgentState, GuestAgentStateView> = {
	active: {
		label: 'Active',
		tone: 'success',
		description: 'The agent is reporting over its enrolled credential.'
	},
	renewal_due: {
		label: 'Rotation due',
		tone: 'warning',
		description:
			'The credential is inside its rotation window; the agent rotates it on the next ingest.'
	},
	offline: {
		label: 'Offline',
		tone: 'muted',
		description: 'No accepted batch within the offline threshold.'
	},
	expired: {
		label: 'Expired',
		tone: 'danger',
		description: 'The credential expired without rotation; re-enroll with a fresh claim.'
	},
	revoked: {
		label: 'Revoked',
		tone: 'danger',
		description: 'An operator revoked this agent; it can no longer report.'
	},
	identity_conflict: {
		label: 'Identity conflict',
		tone: 'danger',
		description:
			'The credential was presented from a different install (cloned image). Reporting is blocked until an authorized reset.'
	},
	unenrolled: {
		label: 'Not enrolled',
		tone: 'muted',
		description: 'No agent is enrolled for this VM; guests stay fully usable without one.'
	},
	enrolling: {
		label: 'Enrolling',
		tone: 'info',
		description: 'A claim was issued and is waiting for redemption.'
	}
};

export function guestAgentStateView(state: GuestAgentState): GuestAgentStateView {
	return STATE_VIEWS[state];
}

/** The credential's remaining lifetime, human-compact. */
export function guestAgentCredentialRemaining(
	expiresAtMs: number,
	nowMs: number = Date.now()
): string {
	const remaining = expiresAtMs - nowMs;
	if (remaining <= 0) return 'expired';
	const days = Math.floor(remaining / 86_400_000);
	if (days >= 1) return `${days}d`;
	const hours = Math.floor(remaining / 3_600_000);
	if (hours >= 1) return `${hours}h`;
	return `${Math.max(1, Math.floor(remaining / 60_000))}m`;
}

/** "last seen" copy; absence is stated honestly, never "0s ago". */
export function guestAgentLastSeen(
	ageSeconds: number | null,
	state: GuestAgentState
): string {
	if (ageSeconds === null) {
		return state === 'revoked' ? 'never reported' : 'awaiting first batch';
	}
	if (ageSeconds < 60) return `${ageSeconds}s ago`;
	if (ageSeconds < 3600) return `${Math.floor(ageSeconds / 60)}m ago`;
	if (ageSeconds < 86_400) return `${Math.floor(ageSeconds / 3600)}h ago`;
	return `${Math.floor(ageSeconds / 86_400)}d ago`;
}

/** The actions an operator may take in a given state. */
export type GuestAgentAction = 'enroll' | 'rotate' | 'revoke' | 'reset';

export function guestAgentActions(state: GuestAgentState): GuestAgentAction[] {
	switch (state) {
		case 'active':
		case 'renewal_due':
			return ['rotate', 'revoke'];
		case 'identity_conflict':
			return ['reset', 'revoke'];
		case 'offline':
		case 'expired':
			return ['revoke'];
		case 'revoked':
			return ['enroll'];
		case 'unenrolled':
		case 'enrolling':
			return ['enroll'];
	}
}
