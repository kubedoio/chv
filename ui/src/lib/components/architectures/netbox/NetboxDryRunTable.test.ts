import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/svelte';
import NetboxDryRunTable from './NetboxDryRunTable.svelte';
import type { NetboxPlanEntry, NetboxProjectionPlan } from '#lib/bff/architectures.ts';

function makeEntry(overrides: Partial<NetboxPlanEntry> = {}): NetboxPlanEntry {
	return {
		action: 'create',
		kind: 'virtual_machine',
		chv_resource_ref: 'instances/app-01',
		netbox_natural_key: { name: 'app-01' },
		external_id: 'arch:arch-1:instance/app-01:3',
		reason: 'no object with this external id; natural key free',
		changes: [],
		...overrides
	};
}

function makePlan(overrides: Partial<NetboxProjectionPlan> = {}): NetboxProjectionPlan {
	return {
		mapping_version: 'v1',
		architecture_id: 'arch-1',
		architecture_version: 3,
		retention: 'mark_stale',
		summary: { create: 1, update: 0, no_op: 0, conflict: 0, stale: 0 },
		entries: [makeEntry()],
		...overrides
	};
}

describe('NetboxDryRunTable', () => {
	afterEach(() => cleanup());

	it('renders the conflict cue on a conflict entry (the mapping contract: never written, CHV modifies only what it owns)', () => {
		const plan = makePlan({
			summary: { create: 0, update: 0, no_op: 0, conflict: 1, stale: 0 },
			entries: [
				makeEntry({
					action: 'conflict',
					kind: 'prefix',
					chv_resource_ref: 'networks/tenant-prod',
					netbox_natural_key: { prefix: '10.0.20.0/24' },
					reason: 'prefix exists and is not owned by chv (chv_managed_by absent)'
				})
			]
		});
		const { getByTestId, queryAllByTestId } = render(NetboxDryRunTable, { props: { plan } });

		const cue = getByTestId('netbox-conflict-cue');
		// Contract-accurate cue: unconditional non-write guarantee, no
		// blanket ownership-marker claim (conflicts also arise on
		// CHV-owned objects — foreign mapping version, charset, …), and
		// the reason field is the authoritative per-entry explanation.
		expect(cue.textContent?.toLowerCase()).toContain('conflict');
		expect(cue.textContent?.toLowerCase()).toContain('not written');
		expect(cue.textContent?.toLowerCase()).toContain('chv never modifies objects it does not own');
		expect(cue.textContent?.toLowerCase()).toContain('reason');
		// Exactly one cue, on the one conflict row.
		expect(queryAllByTestId('netbox-conflict-cue')).toHaveLength(1);
	});

	it('omits the conflict cue on non-conflict entries', () => {
		const plan = makePlan({
			summary: { create: 1, update: 0, no_op: 0, conflict: 0, stale: 0 },
			entries: [makeEntry()]
		});
		const { queryByTestId } = render(NetboxDryRunTable, { props: { plan } });

		expect(queryByTestId('netbox-conflict-cue')).toBeNull();
		expect(queryByTestId('netbox-conflict-banner')).toBeNull();
	});

	it('counts the summary chips correctly and renders the conflict banner when conflicts exist', () => {
		const plan = makePlan({
			summary: { create: 4, update: 1, no_op: 7, conflict: 2, stale: 0 },
			entries: [
				makeEntry({ action: 'conflict' }),
				makeEntry({ chv_resource_ref: 'instances/app-02' })
			]
		});
		const { getByTestId, getAllByTestId } = render(NetboxDryRunTable, { props: { plan } });

		const chips = getAllByTestId('netbox-summary-chip');
		expect(chips).toHaveLength(5);
		const counts = Object.fromEntries(
			chips.map((chip) => [
				chip.getAttribute('data-netbox-action'),
				chip.querySelector('[data-testid="netbox-summary-chip-count"]')?.textContent
			])
		);
		expect(counts).toEqual({ create: '4', update: '1', no_op: '7', conflict: '2', stale: '0' });

		const banner = getByTestId('netbox-conflict-banner');
		expect(banner.getAttribute('role')).toBe('alert');
		expect(banner.textContent).toContain('2 conflicting objects');
		// The non-write guarantee is unconditional; the cause lives in
		// the per-entry reasons, not a blanket ownership-marker claim.
		expect(banner.textContent).toContain('never written');
		expect(banner.textContent).toContain('export proceeds with the remaining entries');
	});

	it('maps action badges to human labels and keeps the server order deterministic', () => {
		const plan = makePlan({
			summary: { create: 1, update: 1, no_op: 1, conflict: 0, stale: 1 },
			entries: [
				makeEntry({ action: 'update', changes: ['memory_gb: 64 → 128'] }),
				makeEntry({ action: 'create' }),
				makeEntry({ action: 'no_op' }),
				makeEntry({ action: 'stale' })
			]
		});
		const { getAllByTestId } = render(NetboxDryRunTable, { props: { plan } });

		const badges = getAllByTestId('netbox-action-badge');
		expect(badges.map((b) => b.textContent?.trim())).toEqual([
			'Update',
			'Create',
			'No change',
			'Stale'
		]);
		// Entry rows stay in the plan's (server-determined) order.
		const entries = getAllByTestId('netbox-plan-entry');
		expect(entries.map((e) => e.getAttribute('data-netbox-entry-action'))).toEqual([
			'update',
			'create',
			'no_op',
			'stale'
		]);
	});

	it('renders field changes only on update entries', () => {
		const plan = makePlan({
			summary: { create: 0, update: 1, no_op: 0, conflict: 0, stale: 0 },
			entries: [makeEntry({ action: 'update', changes: ['memory_gb: 64 → 128'] })]
		});
		const { getAllByTestId } = render(NetboxDryRunTable, { props: { plan } });

		const changes = getAllByTestId('netbox-entry-change');
		expect(changes).toHaveLength(1);
		expect(changes[0].textContent).toBe('memory_gb: 64 → 128');
	});

	it('renders the empty state when the plan has no entries', () => {
		const plan = makePlan({
			summary: { create: 0, update: 0, no_op: 0, conflict: 0, stale: 0 },
			entries: []
		});
		const { getByTestId, queryByTestId } = render(NetboxDryRunTable, { props: { plan } });

		expect(getByTestId('netbox-dry-run-empty')).toBeTruthy();
		expect(queryByTestId('netbox-plan-entry')).toBeNull();
	});
});
