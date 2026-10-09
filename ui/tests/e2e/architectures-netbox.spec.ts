import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer — NetBox projection tab (issue #239, PR 7/8).
 *
 * Same `page.route(...)` mock strategy as the drift spec: the BFF wire
 * is stubbed so the suite does not depend on the Rust handler
 * shipping. The mocks record the latest request body per endpoint so
 * the wire-level assertions (the upsert body's write-only token, the
 * export 409 branch) can inspect what the panel actually sent.
 *
 * Wire shapes under test (match `ui/src/lib/bff/architectures.ts`):
 *   config/get, config/upsert → NetboxConfig (token_set only — never
 *   the token), export/dry-run → the flattened NetboxProjectionPlan,
 *   export → NetboxExportResponse | 409 NETBOX_RUN_ACTIVE,
 *   runs/list → { runs: NetboxRunSummary[] }, runs/get →
 *   NetboxRunDetail, runs/retry → NetboxRunRetryResponse.
 */

type ArchStatus = 'draft' | 'applied' | 'archived';

interface MockArch {
	id: string;
	name: string;
	display_name: string | null;
	description: string | null;
	environment: string | null;
	status: ArchStatus;
	owner_user_id: string | null;
	last_validation_status: 'unknown' | 'passed' | 'failed' | null;
	last_fleet_check_status: 'unknown' | 'passed' | 'failed' | null;
	version_number: number;
	created_at: string;
	updated_at: string;
	archived_at: string | null;
}

interface NetboxConfigResponse {
	architecture_id: string;
	endpoint: string;
	token_secret_ref: string;
	token_set: boolean;
	retention_policy: 'mark_stale' | 'delete';
	enable_post_apply: boolean;
	custom_field_prefix: string;
	site_name: string | null;
	updated_at: string;
}

interface NetboxPlanEntry {
	action: 'create' | 'update' | 'no_op' | 'conflict' | 'stale';
	kind: 'vlan' | 'prefix' | 'ip_address' | 'interface' | 'virtual_machine' | 'device';
	chv_resource_ref: string;
	netbox_natural_key: Record<string, string>;
	external_id: string;
	reason: string;
	changes: string[];
}

interface NetboxProjectionPlanResponse {
	mapping_version: string;
	architecture_id: string;
	architecture_version: number;
	retention: 'mark_stale' | 'delete';
	summary: { create: number; update: number; no_op: number; conflict: number; stale: number };
	entries: NetboxPlanEntry[];
}

interface NetboxRunSummaryResponse {
	id: string;
	architecture_id: string;
	trigger: 'manual' | 'post_apply';
	status: 'queued' | 'running' | 'succeeded' | 'failed';
	mode: 'dry_run' | 'export';
	summary: NetboxProjectionPlanResponse['summary'] | null;
	error_message: string | null;
	attempt_count: number;
	requested_by: string | null;
	started_at: string | null;
	finished_at: string | null;
	created_at: string;
}

const NOW = '2026-10-09T12:00:00Z';

const MOCK_CONFIG: NetboxConfigResponse = {
	architecture_id: 'arch-1',
	endpoint: 'https://netbox.example.internal',
	token_secret_ref: 'netbox-arch-1',
	token_set: true,
	retention_policy: 'mark_stale',
	enable_post_apply: true,
	custom_field_prefix: 'chv_',
	site_name: 'dc1',
	updated_at: NOW
};

class FakeNetboxBackend {
	architectures: MockArch[] = [];
	private idCounter = 0;

	// Latest recorded request bodies (the wire-level assertions).
	lastConfigGetBody: { id?: string } | null = null;
	lastUpsertBody: Record<string, unknown> | null = null;
	lastDryRunBody: { id?: string } | null = null;
	lastExportBody: { id?: string } | null = null;
	lastRetryBody: { id?: string; run_id?: string } | null = null;
	/** The exact config/get response body served (token-exposure check). */
	servedConfigBody: string | null = null;

	// Export responses: each call consumes the head of the queue; once
	// exhausted, the last response repeats (the drift spec's pattern).
	exportQueue: Array<{ status: number; body: unknown }>;

	// Run rows: the failed row flips to queued after a successful retry.
	runs: NetboxRunSummaryResponse[];

	constructor(
		exportQueue: Array<{ status: number; body: unknown }>,
		runs: NetboxRunSummaryResponse[]
	) {
		this.exportQueue = [...exportQueue];
		this.runs = [...runs];
	}

	create(input: { name: string; environment?: string | null }): MockArch {
		this.idCounter += 1;
		const arch: MockArch = {
			id: `arch-${this.idCounter}`,
			name: input.name,
			display_name: null,
			description: null,
			environment: input.environment ?? null,
			status: 'draft',
			owner_user_id: null,
			last_validation_status: null,
			last_fleet_check_status: null,
			version_number: 1,
			created_at: NOW,
			updated_at: NOW,
			archived_at: null
		};
		this.architectures.push(arch);
		return arch;
	}

	nextExport(): { status: number; body: unknown } {
		if (this.exportQueue.length > 1) {
			return this.exportQueue.shift() as { status: number; body: unknown };
		}
		return this.exportQueue[0];
	}
}

async function installArchitectureMocks(page: Page, backend: FakeNetboxBackend) {
	const json = (route: Route, status: number, body: unknown) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(body)
		});

	await page.route('**/v1/architectures/list', async (route) => {
		await json(route, 200, { architectures: backend.architectures });
	});

	await page.route('**/v1/architectures/create', async (route) => {
		const body = route.request().postDataJSON() as { name: string; environment?: string | null };
		const arch = backend.create(body);
		await json(route, 200, { architecture: arch });
	});

	await page.route('**/v1/architectures/get', async (route) => {
		const body = route.request().postDataJSON() as { id: string };
		const arch = backend.architectures.find((a) => a.id === body.id);
		if (!arch) {
			await json(route, 404, { message: 'not found', code: 'NOT_FOUND' });
			return;
		}
		await json(route, 200, {
			architecture: arch,
			design_graph_json: null,
			latest_yaml: null
		});
	});

	await page.route('**/v1/architectures/update', async (route) => {
		const body = route.request().postDataJSON() as { id: string };
		const arch = backend.architectures.find((a) => a.id === body.id);
		if (!arch) {
			await json(route, 404, { message: 'not found', code: 'NOT_FOUND' });
			return;
		}
		await json(route, 200, { architecture: arch });
	});

	// Sidebar fetches that must not 404.
	await page.route('**/v1/nodes', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
	await page.route('**/v1/vms', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
}

async function installNetboxMocks(
	page: Page,
	backend: FakeNetboxBackend,
	options: { config?: NetboxConfigResponse | null; dryRun?: NetboxProjectionPlanResponse } = {}
) {
	const json = (route: Route, status: number, body: unknown) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(body)
		});

	const configResponse = 'config' in options ? options.config : MOCK_CONFIG;

	await page.route('**/v1/architectures/netbox/config/get', async (route) => {
		backend.lastConfigGetBody = route.request().postDataJSON() as { id?: string };
		if (!configResponse) {
			await json(route, 404, { message: 'no config', code: 'NETBOX_NOT_CONFIGURED' });
			return;
		}
		backend.servedConfigBody = JSON.stringify(configResponse);
		await json(route, 200, configResponse);
	});

	await page.route('**/v1/architectures/netbox/config/upsert', async (route) => {
		backend.lastUpsertBody = route.request().postDataJSON() as Record<string, unknown>;
		await json(route, 200, configResponse ?? MOCK_CONFIG);
	});

	await page.route('**/v1/architectures/netbox/export/dry-run', async (route) => {
		backend.lastDryRunBody = route.request().postDataJSON() as { id?: string };
		await json(route, 200, options.dryRun ?? EMPTY_PLAN);
	});

	await page.route('**/v1/architectures/netbox/export', async (route) => {
		backend.lastExportBody = route.request().postDataJSON() as { id?: string };
		const next = backend.nextExport();
		// A 200 queues a run: materialize the row the real control
		// plane would persist, so the store's post-export refresh of
		// runs/list observes it (refreshRunsIfCurrent).
		if (next.status === 200) {
			const body = next.body as { run_id: string };
			backend.runs.unshift({
				id: body.run_id,
				architecture_id: 'arch-1',
				trigger: 'manual',
				status: 'queued',
				mode: 'export',
				summary: null,
				error_message: null,
				attempt_count: 0,
				requested_by: 'senol',
				started_at: null,
				finished_at: null,
				created_at: NOW
			});
		}
		await json(route, next.status, next.body);
	});

	await page.route('**/v1/architectures/netbox/runs/list', async (route) => {
		await json(route, 200, { runs: backend.runs });
	});

	await page.route('**/v1/architectures/netbox/runs/get', async (route) => {
		const body = route.request().postDataJSON() as { run_id?: string };
		const run = backend.runs.find((r) => r.id === body.run_id) ?? backend.runs[0];
		await json(route, 200, {
			id: run.id,
			architecture_id: run.architecture_id,
			architecture_version_id: 'ver-1',
			trigger: run.trigger,
			status: run.status,
			mode: run.mode,
			// The REAL post-fix wire: `plan_json` is null in the current
			// flow (both enqueue sites write None), and the executed-plan
			// chips come from the FLAT outcome the BFF serves after
			// unwrapping the worker's provenance envelope — with the
			// envelope's version id surfaced as a first-class field.
			plan_json: null,
			result_json:
				run.summary === null
					? null
					: {
							plan: { ...EMPTY_PLAN, summary: run.summary },
							entries: [
								{
									action: 'create',
									kind: 'vlan',
									chv_resource_ref: 'networks/backend',
									status: 'succeeded',
									error: null
								}
							],
							summary: { succeeded: 1, failed: 0, skipped: 0, not_attempted: 0 },
							error: null
						},
			resolved_architecture_version_id: 'ver-1',
			summary: run.summary,
			error_message: run.error_message,
			attempt_count: run.attempt_count,
			requested_by: run.requested_by,
			started_at: run.started_at,
			finished_at: run.finished_at,
			next_attempt_at: null,
			created_at: run.created_at
		});
	});

	await page.route('**/v1/architectures/netbox/runs/retry', async (route) => {
		backend.lastRetryBody = route.request().postDataJSON() as { id?: string; run_id?: string };
		// Flip the failed row to queued, like the real BFF + worker would.
		const failed = backend.runs.find((r) => r.status === 'failed');
		if (failed) {
			failed.status = 'queued';
			failed.error_message = null;
		}
		await json(route, 200, { run_id: backend.lastRetryBody.run_id, status: 'queued' });
	});
}

async function createArchitectureAndOpen(page: Page, name: string) {
	await page.goto('/architectures/new');
	await page.locator('#arch-name').fill(name);
	await page.locator('#arch-environment').selectOption('development');
	await page.getByRole('button', { name: /create architecture/i }).click();
	await expect(page).toHaveURL(/\/architectures\/arch-1$/);
}

const EMPTY_PLAN: NetboxProjectionPlanResponse = {
	mapping_version: 'v1',
	architecture_id: 'arch-1',
	architecture_version: 1,
	retention: 'mark_stale',
	summary: { create: 0, update: 0, no_op: 0, conflict: 0, stale: 0 },
	entries: []
};

const WITH_CONFLICT: NetboxProjectionPlanResponse = {
	mapping_version: 'v1',
	architecture_id: 'arch-1',
	architecture_version: 2,
	retention: 'mark_stale',
	summary: { create: 1, update: 1, no_op: 2, conflict: 1, stale: 0 },
	entries: [
		{
			action: 'create',
			kind: 'prefix',
			chv_resource_ref: 'networks/frontend',
			netbox_natural_key: { prefix: '10.7.0.0/24' },
			external_id: 'arch:arch-1:network/frontend:2',
			reason: 'natural key is free and no remote object carries this external id',
			changes: []
		},
		{
			action: 'update',
			kind: 'virtual_machine',
			chv_resource_ref: 'instances/vm-01',
			netbox_natural_key: { name: 'vm-01' },
			external_id: 'arch:arch-1:instance/vm-01:2',
			reason: 'chv-owned remote object differs from the desired projection',
			changes: ['memory_mb: 2048 → 4096']
		},
		{
			action: 'no_op',
			kind: 'vlan',
			chv_resource_ref: 'networks/backend',
			netbox_natural_key: { vid: '42' },
			external_id: 'arch:arch-1:network/backend#vlan:2',
			reason: 'remote object matches the desired projection',
			changes: []
		},
		{
			action: 'no_op',
			kind: 'device',
			chv_resource_ref: 'servers/chv-node-01',
			netbox_natural_key: { name: 'chv-node-01' },
			external_id: 'arch:arch-1:server/chv-node-01:2',
			reason: 'remote object matches the desired projection',
			changes: []
		},
		{
			action: 'conflict',
			kind: 'ip_address',
			chv_resource_ref: 'instances/vm-01/backend',
			netbox_natural_key: { address: '10.42.0.5' },
			external_id: 'arch:arch-1:instance/vm-01/backend#10.42.0.5:2',
			reason: 'natural key occupied by an object not owned by this chv architecture',
			changes: []
		}
	]
};

const RUN_SUCCEEDED: NetboxRunSummaryResponse = {
	id: 'netrun-ok',
	architecture_id: 'arch-1',
	trigger: 'post_apply',
	status: 'succeeded',
	mode: 'export',
	summary: { create: 6, update: 0, no_op: 0, conflict: 0, stale: 0 },
	error_message: null,
	attempt_count: 1,
	requested_by: null,
	started_at: NOW,
	finished_at: NOW,
	created_at: NOW
};

const RUN_FAILED: NetboxRunSummaryResponse = {
	id: 'netrun-failed',
	architecture_id: 'arch-1',
	trigger: 'manual',
	status: 'failed',
	mode: 'export',
	summary: null,
	error_message: 'netbox unreachable: connection refused',
	attempt_count: 3,
	requested_by: 'senol',
	started_at: NOW,
	finished_at: NOW,
	created_at: NOW
};

test.describe('Architecture Designer — NetBox projection tab', () => {
	test('netbox-config-form-renders-from-mocked-config', async ({ page }) => {
		const backend = new FakeNetboxBackend([], []);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);
		await installNetboxMocks(page, backend);

		await createArchitectureAndOpen(page, 'netbox-config');
		await page.getByTestId('tab-netbox').click();

		// Panel mounts and lazy-loads the config + run history.
		await expect(page.getByTestId('netbox-panel')).toBeVisible();
		await expect(page.getByTestId('netbox-config-form')).toBeVisible();

		// The form seeds from the mocked config/get: endpoint, prefix,
		// retention, post-apply toggle, site.
		await expect(page.getByTestId('netbox-endpoint-input')).toHaveValue(
			'https://netbox.example.internal'
		);
		await expect(page.getByTestId('netbox-custom-field-prefix')).toContainText('chv_');
		await expect(page.getByTestId('netbox-retention-input')).toHaveValue('mark_stale');
		await expect(page.getByTestId('netbox-post-apply-input')).toBeChecked();
		await expect(page.getByTestId('netbox-site-input')).toHaveValue('dc1');

		// Token field is write-only: password-typed, empty, and its
		// placeholder states a token is already set.
		const tokenInput = page.getByTestId('netbox-token-input');
		await expect(tokenInput).toHaveValue('');
		await expect(tokenInput).toHaveAttribute('type', 'password');
		await expect(tokenInput).toHaveAttribute(
			'placeholder',
			/Token set — leave blank to keep the existing token/
		);
		await expect(page.getByTestId('netbox-token-help')).toContainText(/never displayed again/);

		// Initial-mount request shape: config/get + runs/list with the
		// architecture id.
		expect(backend.lastConfigGetBody?.id).toBe('arch-1');

		// Axe scan scoped to the NetBox panel.
		const results = await new AxeBuilder({ page })
			.disableRules(['color-contrast', 'region'])
			.include('[data-testid="netbox-panel"]')
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		const blocking = results.violations.filter(
			(v) => v.impact === 'serious' || v.impact === 'critical'
		);
		expect(blocking).toEqual([]);
	});

	test('netbox-dry-run-renders-entries-chips-and-conflict-cue', async ({ page }) => {
		const backend = new FakeNetboxBackend([], []);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);
		await installNetboxMocks(page, backend, { dryRun: WITH_CONFLICT });

		await createArchitectureAndOpen(page, 'netbox-dryrun');
		await page.getByTestId('tab-netbox').click();
		await expect(page.getByTestId('netbox-panel')).toBeVisible();

		// Before the run: the empty hint, no table.
		await expect(page.getByTestId('netbox-dry-run-empty')).toBeVisible();

		await page.getByTestId('netbox-dry-run-button').click();

		// The table renders every plan entry in plan order.
		await expect(page.getByTestId('netbox-dry-run-table')).toBeVisible();
		await expect(page.getByTestId('netbox-plan-entry')).toHaveCount(5);
		await expect(page.getByTestId('netbox-plan-entry').first()).toHaveAttribute(
			'data-netbox-entry-action',
			'create'
		);
		await expect(page.getByText(/memory_mb: 2048 → 4096/)).toBeVisible();

		// The five summary chips render with the mocked counts.
		await expect(page.getByTestId('netbox-summary-chip')).toHaveCount(5);
		const conflictChip = page.locator(
			'[data-testid="netbox-summary-chip"][data-netbox-action="conflict"]'
		);
		await expect(conflictChip.getByTestId('netbox-summary-chip-count')).toHaveText('1');

		// The conflict entry carries the ownership-conflict cue and the
		// conflict banner explains that conflicts are never written.
		const conflictEntry = page.locator(
			'[data-testid="netbox-plan-entry"][data-netbox-entry-action="conflict"]'
		);
		await expect(conflictEntry).toHaveCount(1);
		await expect(conflictEntry.getByTestId('netbox-conflict-cue')).toBeVisible();
		await expect(page.getByTestId('netbox-conflict-banner')).toBeVisible();
		await expect(page.getByTestId('netbox-conflict-banner')).toContainText(/never written/i);

		// The dry-run request carried the architecture id.
		expect(backend.lastDryRunBody?.id).toBe('arch-1');
	});

	test('netbox-run-history-renders-rows-retry-and-detail', async ({ page }) => {
		const backend = new FakeNetboxBackend([], [RUN_SUCCEEDED, RUN_FAILED]);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);
		await installNetboxMocks(page, backend);

		await createArchitectureAndOpen(page, 'netbox-runs');
		await page.getByTestId('tab-netbox').click();

		// Both rows render; only the failed one exposes the retry
		// affordance and its error message.
		await expect(page.getByTestId('netbox-run-row')).toHaveCount(2);
		await expect(page.getByTestId('netbox-run-retry-button')).toHaveCount(1);
		await expect(page.getByTestId('netbox-run-error')).toContainText(/netbox unreachable/);
		await expect(page.getByTestId('netbox-run-summary').first()).toContainText(/6 create/);

		// Selecting a row renders the run detail with its per-entry
		// outcomes and executed-plan chips. The chips come from the
		// outcome's plan.summary inside the flat result_json (the mock
		// serves plan_json: null — the real wire), so the counts prove
		// the real data flow.
		await page.getByTestId('netbox-run-row-select').first().click();
		await expect(page.getByTestId('netbox-run-detail')).toBeVisible();
		await expect(page.getByTestId('netbox-run-outcome')).toHaveCount(1);
		await expect(page.getByTestId('netbox-run-resolved-version')).toHaveText('ver-1');
		await expect(page.getByTestId('netbox-executed-plan')).toBeVisible();
		const executedChips = page.getByTestId('netbox-executed-plan-chip');
		await expect(executedChips).toHaveCount(5);
		await expect(
			page.locator('[data-testid="netbox-executed-plan-chip"][data-netbox-action="create"] .chip-count')
		).toHaveText('6');

		// Retry: the failed row flips back to queued after the BFF
		// acknowledges, and the request carried the run id.
		await page.getByTestId('netbox-run-retry-button').click();
		const retriedRow = page.locator(
			'[data-testid="netbox-run-row"][data-netbox-run-status="queued"]'
		);
		await expect(retriedRow).toHaveCount(1);
		expect(backend.lastRetryBody?.id).toBe('arch-1');
		expect(backend.lastRetryBody?.run_id).toBe('netrun-failed');
	});

	test('netbox-export-happy-path-then-run-active-409-banner', async ({ page }) => {
		const queuedRun: NetboxRunSummaryResponse = {
			id: 'netrun-new',
			architecture_id: 'arch-1',
			trigger: 'manual',
			status: 'queued',
			mode: 'export',
			summary: null,
			error_message: null,
			attempt_count: 0,
			requested_by: 'senol',
			started_at: null,
			finished_at: null,
			created_at: NOW
		};
		const backend = new FakeNetboxBackend(
			[
				{ status: 200, body: { run_id: 'netrun-new', architecture_id: 'arch-1', status: 'queued' } },
				{
					status: 409,
					body: {
						message: 'a projection run is already queued or running',
						code: 'NETBOX_RUN_ACTIVE'
					}
				}
			],
			[RUN_SUCCEEDED]
		);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);
		await installNetboxMocks(page, backend);

		await createArchitectureAndOpen(page, 'netbox-export');
		await page.getByTestId('tab-netbox').click();

		// Happy path (development topology → no confirmation dialog):
		// the export is queued and the refreshed history shows the new
		// queued run.
		await page.getByTestId('netbox-export-button').click();
		await expect(
			page.locator('[data-testid="netbox-run-row"][data-netbox-run-status="queued"]')
		).toHaveCount(1);
		expect(backend.lastExportBody?.id).toBe('arch-1');
		await expect(page.getByTestId('netbox-export-error')).toHaveCount(0);

		// Second click hits the mocked 409 NETBOX_RUN_ACTIVE branch:
		// the inline banner shows the actionable text and the code.
		await page.getByTestId('netbox-export-button').click();
		const banner = page.getByTestId('netbox-export-error');
		await expect(banner).toBeVisible();
		await expect(banner).toContainText(/already queued or running/i);
		await expect(banner).toContainText(/wait for it to finish before exporting again/i);
		await expect(banner).toHaveAttribute('data-netbox-error-code', 'NETBOX_RUN_ACTIVE');
	});

	test('netbox-config-save-carries-write-only-token', async ({ page }) => {
		const backend = new FakeNetboxBackend([], []);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);
		await installNetboxMocks(page, backend);

		await createArchitectureAndOpen(page, 'netbox-token');
		await page.getByTestId('tab-netbox').click();
		await expect(page.getByTestId('netbox-config-form')).toBeVisible();

		// The config the mock serves (and hence the page renders from)
		// exposes only `token_set` — no token material anywhere in the
		// response body.
		expect(backend.servedConfigBody).not.toBeNull();
		const served = JSON.parse(backend.servedConfigBody as string) as Record<string, unknown>;
		expect(served.token_set).toBe(true);
		expect(Object.keys(served).sort()).toEqual([
			'architecture_id',
			'custom_field_prefix',
			'enable_post_apply',
			'endpoint',
			'retention_policy',
			'site_name',
			'token_secret_ref',
			'token_set',
			'updated_at'
		]);

		// Saving with a typed token: the upsert body carries the token
		// exactly once, alongside the architecture id and expected
		// version — the write-only direction of the contract.
		await page.getByTestId('netbox-token-input').fill('netbox-secret-token-do-not-log-3f9a');
		await page.getByTestId('netbox-config-save-button').click();

		await expect(page.getByTestId('netbox-token-input')).toHaveValue('');
		expect(backend.lastUpsertBody).not.toBeNull();
		expect(backend.lastUpsertBody?.token).toBe('netbox-secret-token-do-not-log-3f9a');
		expect(backend.lastUpsertBody?.id).toBe('arch-1');
		expect(backend.lastUpsertBody?.expected_version).toBe(1);

		// After the save the form re-seeds from the server summary and
		// never echoes the token back into the DOM.
		await expect(page.getByTestId('netbox-token-input')).toHaveAttribute(
			'placeholder',
			/Token set — leave blank to keep the existing token/
		);
		const tokenValue = await page
			.getByTestId('netbox-token-input')
			.inputValue()
			.catch(() => '');
		expect(tokenValue).toBe('');
	});
});
