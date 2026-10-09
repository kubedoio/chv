import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import NetboxRunHistory from './NetboxRunHistory.svelte';
import type { NetboxRunDetail, NetboxRunSummary } from '#lib/bff/architectures.ts';

function makeRun(overrides: Partial<NetboxRunSummary> = {}): NetboxRunSummary {
	return {
		id: 'netrun-1',
		architecture_id: 'arch-1',
		trigger: 'manual',
		status: 'succeeded',
		mode: 'export',
		summary: { create: 1, update: 0, no_op: 0, conflict: 0, stale: 0 },
		error_message: null,
		attempt_count: 1,
		requested_by: 'user-1',
		started_at: '2026-10-08T09:01:00Z',
		finished_at: '2026-10-08T09:01:05Z',
		created_at: '2026-10-08T09:01:00Z',
		...overrides
	};
}

const RUN_DETAIL: NetboxRunDetail = {
	id: 'netrun-1',
	architecture_id: 'arch-1',
	architecture_version_id: 'ver-3',
	trigger: 'manual',
	status: 'succeeded',
	mode: 'export',
	plan_json: null,
	result_json: {
		plan: {
			mapping_version: 'v1',
			architecture_id: 'arch-1',
			architecture_version: 3,
			retention: 'mark_stale',
			summary: { create: 1, update: 0, no_op: 0, conflict: 0, stale: 0 },
			entries: []
		},
		entries: [
			{
				action: 'create',
				kind: 'virtual_machine',
				chv_resource_ref: 'instances/app-01',
				status: 'succeeded',
				error: null
			},
			{
				action: 'conflict',
				kind: 'prefix',
				chv_resource_ref: 'networks/tenant-prod',
				status: 'skipped',
				error: null
			}
		],
		summary: { succeeded: 1, failed: 0, skipped: 1, not_attempted: 0 },
		error: null
	},
	summary: { create: 1, update: 0, no_op: 0, conflict: 0, stale: 0 },
	error_message: null,
	attempt_count: 1,
	requested_by: 'user-1',
	started_at: '2026-10-08T09:01:00Z',
	finished_at: '2026-10-08T09:01:05Z',
	next_attempt_at: null,
	created_at: '2026-10-08T09:01:00Z'
};

function renderHistory(overrides: Record<string, unknown> = {}) {
	const onSelectRun = vi.fn();
	const onRetry = vi.fn();
	const rendered = render(NetboxRunHistory, {
		props: {
			runs: [makeRun()],
			loading: false,
			currentRun: null,
			runLoading: false,
			retrying: false,
			onSelectRun,
			onRetry,
			...overrides
		}
	});
	return { ...rendered, onSelectRun, onRetry };
}

describe('NetboxRunHistory', () => {
	afterEach(() => cleanup());

	it('renders one row per run with status, trigger and mode', () => {
		const { getAllByTestId } = renderHistory({
			runs: [makeRun(), makeRun({ id: 'netrun-2', status: 'failed', trigger: 'post_apply' })]
		});

		const rows = getAllByTestId('netbox-run-row');
		expect(rows).toHaveLength(2);
		expect(rows[0].getAttribute('data-netbox-run-status')).toBe('succeeded');
		expect(rows[1].getAttribute('data-netbox-run-status')).toBe('failed');
		expect(rows[1].textContent).toContain('Post-apply');
	});

	it('renders the retry button only on failed runs and forwards the run id', async () => {
		const { getAllByTestId, onRetry } = renderHistory({
			runs: [makeRun(), makeRun({ id: 'netrun-2', status: 'failed' })]
		});

		const rows = getAllByTestId('netbox-run-row');
		expect(rows[0].querySelector('[data-testid="netbox-run-retry-button"]')).toBeNull();
		const retryButton = rows[1].querySelector('[data-testid="netbox-run-retry-button"]');
		expect(retryButton).toBeTruthy();

		await fireEvent.click(retryButton as HTMLElement);
		expect(onRetry).toHaveBeenCalledWith('netrun-2');
	});

	it('renders the run error message inline on the row', () => {
		const { getByTestId } = renderHistory({
			runs: [makeRun({ status: 'failed', error_message: 'netbox unreachable' })]
		});

		expect(getByTestId('netbox-run-error').textContent).toContain('netbox unreachable');
	});

	it('loads the full run detail on row selection and renders per-entry outcomes', async () => {
		const { getByTestId, onSelectRun } = renderHistory();

		await fireEvent.click(getByTestId('netbox-run-row-select'));
		expect(onSelectRun).toHaveBeenCalledWith('netrun-1');

		// The detail view is driven by the `currentRun` prop (the panel
		// wires it to the store); render it with the loaded detail.
		const detail = renderHistory({ currentRun: RUN_DETAIL });
		const outcomes = detail.getAllByTestId('netbox-run-outcome');
		expect(outcomes).toHaveLength(2);
		expect(outcomes[0].getAttribute('data-netbox-outcome-status')).toBe('succeeded');
		expect(outcomes[0].textContent).toContain('instances/app-01');
		// Conflicts execute as skipped entries — never written.
		expect(outcomes[1].getAttribute('data-netbox-outcome-status')).toBe('skipped');
		expect(outcomes[1].textContent).toContain('networks/tenant-prod');
	});

	it('renders the raw result payload when result_json did not parse as structured outcomes', () => {
		const { getByTestId } = renderHistory({
			currentRun: { ...RUN_DETAIL, result_json: 'not-json-but-a-raw-column-string' }
		});

		expect(getByTestId('netbox-run-detail-raw').textContent).toContain('not-json-but-a-raw-column-string');
	});

	it('renders the executed plan summary chips when plan_json is the parsed plan shape', () => {
		const plan = {
			mapping_version: 'v1',
			architecture_id: 'arch-1',
			architecture_version: 3,
			retention: 'mark_stale' as const,
			summary: { create: 2, update: 1, no_op: 0, conflict: 1, stale: 0 },
			entries: []
		};

		const { getByTestId, getAllByTestId } = renderHistory({
			currentRun: { ...RUN_DETAIL, plan_json: plan }
		});

		const section = getByTestId('netbox-executed-plan');
		expect(section.textContent).toContain('Executed plan');
		const chips = getAllByTestId('netbox-executed-plan-chip');
		expect(chips).toHaveLength(5);
		const counts = Object.fromEntries(
			chips.map((chip) => [
				chip.getAttribute('data-netbox-action'),
				chip.querySelector('.chip-count')?.textContent
			])
		);
		expect(counts).toEqual({ create: '2', update: '1', no_op: '0', conflict: '1', stale: '0' });
	});

	it('skips the executed plan section when plan_json is a raw string or null', () => {
		const raw = renderHistory({
			currentRun: { ...RUN_DETAIL, plan_json: 'raw-plan-column-string' }
		});
		expect(raw.queryByTestId('netbox-executed-plan')).toBeNull();

		// RUN_DETAIL carries plan_json: null.
		const none = renderHistory({ currentRun: RUN_DETAIL });
		expect(none.queryByTestId('netbox-executed-plan')).toBeNull();
	});

	it('renders the abort error with the failed resource ref', () => {
		const { getByTestId } = renderHistory({
			currentRun: {
				...RUN_DETAIL,
				result_json: {
					plan: RUN_DETAIL.result_json &&
						typeof RUN_DETAIL.result_json !== 'string'
						? RUN_DETAIL.result_json.plan
						: null,
					entries: [],
					summary: { succeeded: 0, failed: 1, skipped: 0, not_attempted: 2 },
					error: {
						message: 'netbox unreachable',
						failed_chv_resource_ref: 'instances/app-02',
						retryable: true
					}
				}
			}
		});

		const abort = getByTestId('netbox-run-abort-error');
		expect(abort.textContent).toContain('netbox unreachable');
		expect(abort.textContent).toContain('instances/app-02');
	});

	it('renders the empty state and loading state', () => {
		const empty = renderHistory({ runs: [] });
		expect(empty.getByTestId('netbox-runs-empty')).toBeTruthy();

		const loading = renderHistory({ loading: true });
		expect(loading.getByTestId('netbox-runs-loading')).toBeTruthy();
	});
});
