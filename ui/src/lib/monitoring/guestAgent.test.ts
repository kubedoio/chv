import { describe, expect, it } from 'vitest';
import {
	guestAgentActions,
	guestAgentCredentialRemaining,
	guestAgentLastSeen,
	guestAgentStateView
} from './guestAgent.ts';
import type { GuestAgentState } from '#lib/bff/monitoringAgents.ts';

describe('guestAgentStateView', () => {
	it('covers the full wire vocabulary', () => {
		const states: GuestAgentState[] = [
			'active',
			'renewal_due',
			'offline',
			'expired',
			'revoked',
			'identity_conflict',
			'unenrolled',
			'enrolling'
		];
		for (const state of states) {
			const view = guestAgentStateView(state);
			expect(view.label.length).toBeGreaterThan(0);
			expect(view.description.length).toBeGreaterThan(0);
		}
	});

	it('marks the cloned-image signal as danger', () => {
		expect(guestAgentStateView('identity_conflict').tone).toBe('danger');
	});
});

describe('guestAgentActions', () => {
	it('offers revoke and rotate for a healthy agent', () => {
		expect(guestAgentActions('active')).toEqual(['rotate', 'revoke']);
	});

	it('offers reset first for a conflict-flagged agent', () => {
		expect(guestAgentActions('identity_conflict')).toEqual(['reset', 'revoke']);
	});

	it('offers enrollment for revoked and unenrolled VMs', () => {
		expect(guestAgentActions('revoked')).toEqual(['enroll']);
		expect(guestAgentActions('unenrolled')).toEqual(['enroll']);
	});
});

describe('guestAgentCredentialRemaining', () => {
	it('formats days, hours, minutes and expiry', () => {
		const now = 1_000_000_000_000;
		expect(guestAgentCredentialRemaining(now - 1, now)).toBe('expired');
		expect(guestAgentCredentialRemaining(now + 90 * 86_400_000, now)).toBe('90d');
		expect(guestAgentCredentialRemaining(now + 2 * 3_600_000, now)).toBe('2h');
		expect(guestAgentCredentialRemaining(now + 5 * 60_000, now)).toBe('5m');
	});
});

describe('guestAgentLastSeen', () => {
	it('states absence honestly instead of zeroing', () => {
		expect(guestAgentLastSeen(null, 'active')).toBe('awaiting first batch');
		expect(guestAgentLastSeen(null, 'revoked')).toBe('never reported');
	});

	it('formats ages compactly', () => {
		expect(guestAgentLastSeen(42, 'active')).toBe('42s ago');
		expect(guestAgentLastSeen(125, 'active')).toBe('2m ago');
		expect(guestAgentLastSeen(7_200, 'active')).toBe('2h ago');
		expect(guestAgentLastSeen(172_800, 'active')).toBe('2d ago');
	});
});
