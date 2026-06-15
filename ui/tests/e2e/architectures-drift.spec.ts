import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer Phase 6 — drift detection tab.
 *
 * Same `page.route(...)` mock strategy as the fleet/plan specs: stub the
 * BFF wire so the suite does not depend on the Rust handler shipping. The
 * mocks track the latest request body so the third test can assert the
 * `force_refresh` flag flipped to true on a Refresh click.
 *
 * Wire shape under test (matches `getArchitectureDrift` in
 * `ui/src/lib/bff/architectures.ts`):
 *   {
 *     drift_report_id, status, findings, summary,
 *     baseline_version_id, computed_at, cache_hit, error_message
 *   }
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

type DriftStatus = 'unknown' | 'no_drift' | 'drifted' | 'check_failed';

type DriftFinding =
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
	  };

interface DriftResponse {
	drift_report_id: string | null;
	status: DriftStatus;
	findings: DriftFinding[];
	summary: { total: number; by_type: Record<string, number> };
	baseline_version_id: string;
	computed_at: string;
	cache_hit: boolean;
	error_message: string | null;
}

class FakeBackend {
	architectures: MockArch[] = [];
	private idCounter = 0;
	driftQueue: DriftResponse[];
	lastDriftBody: { id?: string; force_refresh?: boolean } | null = null;

	constructor(initialDrift: DriftResponse[]) {
		// Each call to /v1/architectures/drift consumes the head of this queue;
		// once exhausted, the last response is repeated. This lets the third
		// test queue different payloads for the initial load vs the refresh.
		this.driftQueue = [...initialDrift];
	}

	create(input: {
		name: string;
		description?: string | null;
		environment?: string | null;
		display_name?: string | null;
	}): MockArch {
		this.idCounter += 1;
		const now = new Date().toISOString();
		const arch: MockArch = {
			id: `arch-${this.idCounter}`,
			name: input.name,
			display_name: input.display_name ?? null,
			description: input.description ?? null,
			environment: input.environment ?? null,
			status: 'draft',
			owner_user_id: null,
			last_validation_status: null,
			last_fleet_check_status: null,
			version_number: 1,
			created_at: now,
			updated_at: now,
			archived_at: null
		};
		this.architectures.push(arch);
		return arch;
	}

	get(id: string): MockArch | undefined {
		return this.architectures.find((a) => a.id === id);
	}

	nextDrift(): DriftResponse {
		if (this.driftQueue.length > 1) {
			return this.driftQueue.shift() as DriftResponse;
		}
		// Queue holds the steady-state response; repeat without draining.
		return this.driftQueue[0];
	}
}

async function installArchitectureMocks(page: Page, backend: FakeBackend) {
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
		const body = route.request().postDataJSON() as {
			name: string;
			description?: string | null;
			environment?: string | null;
			display_name?: string | null;
		};
		const arch = backend.create(body);
		await json(route, 200, { architecture: arch });
	});

	await page.route('**/v1/architectures/get', async (route) => {
		const body = route.request().postDataJSON() as { id: string };
		const arch = backend.get(body.id);
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
		const arch = backend.get(body.id);
		if (!arch) {
			await json(route, 404, { message: 'not found', code: 'NOT_FOUND' });
			return;
		}
		await json(route, 200, { architecture: arch });
	});

	await page.route('**/v1/architectures/drift', async (route) => {
		const body = route.request().postDataJSON() as { id?: string; force_refresh?: boolean };
		backend.lastDriftBody = body;
		await json(route, 200, backend.nextDrift());
	});

	// Sidebar fetches that must not 404.
	await page.route('**/v1/nodes', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
	await page.route('**/v1/vms', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
}

async function createArchitectureAndOpen(page: Page, name: string) {
	await page.goto('/architectures/new');
	await page.locator('#arch-name').fill(name);
	await page.locator('#arch-environment').selectOption('development');
	await page.getByRole('button', { name: /create architecture/i }).click();
	await expect(page).toHaveURL(/\/architectures\/arch-1$/);
}

const NO_DRIFT: DriftResponse = {
	drift_report_id: 'drift-1',
	status: 'no_drift',
	findings: [],
	summary: { total: 0, by_type: {} },
	baseline_version_id: 'ver-1',
	computed_at: new Date().toISOString(),
	cache_hit: false,
	error_message: null
};

const WITH_FINDINGS: DriftResponse = {
	drift_report_id: 'drift-2',
	status: 'drifted',
	findings: [
		{
			code: 'DRIFT_NETWORK_CHANGED',
			path: 'networks[0]',
			resource_ref: 'networks/lan',
			field: 'vlan_id',
			expected: '100',
			actual: '200',
			message: 'VLAN id changed from 100 to 200'
		},
		{
			code: 'DRIFT_CAPACITY_CHANGED',
			path: 'servers[0].cpu_cores',
			resource_ref: 'servers/host-a',
			field: 'cpu_cores',
			expected: 16,
			actual: 8,
			message: 'host-a cpu_cores reduced from 16 to 8'
		}
	],
	summary: { total: 2, by_type: { DRIFT_NETWORK_CHANGED: 1, DRIFT_CAPACITY_CHANGED: 1 } },
	baseline_version_id: 'ver-1',
	computed_at: new Date().toISOString(),
	cache_hit: false,
	error_message: null
};

const REFRESHED_WITH_ONE_FINDING: DriftResponse = {
	drift_report_id: 'drift-3',
	status: 'drifted',
	findings: [
		{
			code: 'DRIFT_MISSING_RESOURCE',
			path: 'instances[0]',
			resource_ref: 'instances/web',
			message: 'instance web not present in live snapshot'
		}
	],
	summary: { total: 1, by_type: { DRIFT_MISSING_RESOURCE: 1 } },
	baseline_version_id: 'ver-1',
	computed_at: new Date().toISOString(),
	cache_hit: false,
	error_message: null
};

test.describe('Architecture Designer — Phase 6 drift detection', () => {
	test('drift-no-drift-shows-clean-banner', async ({ page }) => {
		const backend = new FakeBackend([NO_DRIFT]);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'drift-clean');
		await page.getByTestId('tab-drift').click();

		// Panel mounts and lazy-loads the cached "no_drift" report.
		await expect(page.getByTestId('drift-report-panel')).toBeVisible();
		await expect(page.getByTestId('drift-empty-state')).toBeVisible();

		// Banner reads "No drift detected".
		await expect(page.getByTestId('drift-status-banner')).toContainText(/no drift detected/i);

		// And no finding rows are rendered.
		await expect(page.getByTestId('drift-finding-row')).toHaveCount(0);

		// Initial-mount request shape: the panel should call /v1/architectures/drift
		// with `force_refresh: false` and the architecture id we just created.
		// FakeBackend records the latest body on `lastDriftBody`.
		expect(backend.lastDriftBody).not.toBeNull();
		expect(backend.lastDriftBody?.force_refresh).toBe(false);
		expect(backend.lastDriftBody?.id).toBe('arch-1');

		// Axe scan scoped to the drift panel.
		const results = await new AxeBuilder({ page })
			.disableRules(['color-contrast', 'region'])
			.include('[data-testid="drift-report-panel"]')
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		const blocking = results.violations.filter(
			(v) => v.impact === 'serious' || v.impact === 'critical'
		);
		expect(blocking).toEqual([]);
	});

	test('drift-with-findings-shows-grouped-list', async ({ page }) => {
		const backend = new FakeBackend([WITH_FINDINGS]);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'drift-found');
		await page.getByTestId('tab-drift').click();

		// Status banner shows "Drifted".
		await expect(page.getByTestId('drift-status-banner')).toContainText(/drift findings/i);

		// Two finding rows in the list.
		await expect(page.getByTestId('drift-finding-row')).toHaveCount(2);

		// The summary chip strip renders one chip per code (active counts > 0
		// for the two emitted codes). All 7 chips render but only the two
		// active ones carry non-zero counts.
		const chips = page.getByTestId('drift-summary-chip');
		await expect(chips).toHaveCount(7);

		// Finding messages are visible verbatim.
		await expect(page.getByText(/VLAN id changed from 100 to 200/i)).toBeVisible();
		await expect(page.getByText(/host-a cpu_cores reduced from 16 to 8/i)).toBeVisible();
	});

	test('drift-refresh-button-fetches-fresh-report', async ({ page }) => {
		// First call returns 0 findings; second call (after Refresh click)
		// returns 1 finding. The queue drains in order — once exhausted the
		// last response is repeated.
		const backend = new FakeBackend([NO_DRIFT, REFRESHED_WITH_ONE_FINDING]);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'drift-refresh');
		await page.getByTestId('tab-drift').click();

		// Initial state: clean banner, no rows.
		await expect(page.getByTestId('drift-empty-state')).toBeVisible();
		await expect(page.getByTestId('drift-finding-row')).toHaveCount(0);

		// Click refresh — store calls getArchitectureDrift(id, true).
		await page.getByTestId('drift-refresh-button').click();

		// UI updates with the refreshed payload.
		await expect(page.getByTestId('drift-finding-row')).toHaveCount(1);
		await expect(page.getByText(/instance web not present in live snapshot/i)).toBeVisible();

		// Body of the second call carried force_refresh=true.
		expect(backend.lastDriftBody?.force_refresh).toBe(true);
	});

	test('drift-check-failed-shows-error-banner', async ({ page }) => {
		// `check_failed` comes back as a normal 200 with status='check_failed'
		// and an `error_message` populated by the BFF (snapshot capture or
		// YAML parse failure). The panel must banner the message verbatim
		// and hide the finding-list/empty-state surface.
		const CHECK_FAILED: DriftResponse = {
			drift_report_id: 'drift-failed-1',
			status: 'check_failed',
			findings: [],
			summary: { total: 0, by_type: {} },
			baseline_version_id: 'ver-1',
			computed_at: new Date().toISOString(),
			cache_hit: false,
			error_message: 'fleet inventory snapshot timed out after 30s'
		};

		const backend = new FakeBackend([CHECK_FAILED]);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'drift-failed');
		await page.getByTestId('tab-drift').click();

		// The failure banner is rendered with the BFF's error_message verbatim.
		const failedBanner = page.getByTestId('drift-failed-banner');
		await expect(failedBanner).toBeVisible();
		await expect(failedBanner).toContainText(/drift check failed/i);
		await expect(failedBanner).toContainText(/fleet inventory snapshot timed out after 30s/);

		// Finding-list state is hidden (no rows, no clean-state empty banner).
		await expect(page.getByTestId('drift-finding-row')).toHaveCount(0);
		await expect(page.getByTestId('drift-empty-state')).toHaveCount(0);
		await expect(page.getByTestId('drift-status-banner')).toHaveCount(0);
	});
});
