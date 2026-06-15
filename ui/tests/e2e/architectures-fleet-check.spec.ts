import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer Phase 3 — Layer 2 fleet-consistency check tab.
 *
 * The fleet tab is always visible (not gated by the Phase-2 canvas flag) so
 * operators can trigger a check even on a Phase-1 imported topology. We use
 * the same `page.route(...)` interception strategy as
 * `architectures-canvas.spec.ts` so this suite does not depend on the Rust
 * BFF endpoint having compiled — only on the documented wire shape.
 *
 * Wire shape under test (matches Phase 1 + the `last_fleet_check_status`
 * column added in Phase 0):
 *   - List   -> `{ architectures: [...] }`
 *   - Get    -> `{ architecture, design_graph_json, latest_yaml }`
 *   - Update body is FLAT.
 *   - check-fleet -> `{ status, inventory_snapshot_id, checked_at, findings }`.
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

interface FleetFinding {
	severity: 'error' | 'warning' | 'info';
	code: string;
	message: string;
	path: string;
	resource_ref: string | null;
	blocking: boolean;
	suggestion: string | null;
}

interface FleetCheckResponse {
	status: 'valid' | 'warning' | 'invalid';
	inventory_snapshot_id: string;
	checked_at: string;
	findings: FleetFinding[];
}

class FakeBackend {
	architectures: MockArch[] = [];
	private idCounter = 0;
	fleetResponse: FleetCheckResponse;

	constructor(initialFleet: FleetCheckResponse) {
		this.fleetResponse = initialFleet;
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
		// Phase 3 fleet-check spec doesn't exercise updates; pass through with
		// a no-op success so any incidental refresh stays green.
		const body = route.request().postDataJSON() as { id: string };
		const arch = backend.get(body.id);
		if (!arch) {
			await json(route, 404, { message: 'not found', code: 'NOT_FOUND' });
			return;
		}
		await json(route, 200, { architecture: arch });
	});

	await page.route('**/v1/architectures/check-fleet', async (route) => {
		await json(route, 200, backend.fleetResponse);
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

const HAPPY_FLEET: FleetCheckResponse = {
	status: 'valid',
	inventory_snapshot_id: 'snap-1',
	checked_at: new Date().toISOString(),
	findings: []
};

const BLOCKED_FLEET: FleetCheckResponse = {
	status: 'invalid',
	inventory_snapshot_id: 'snap-2',
	checked_at: new Date().toISOString(),
	findings: [
		{
			severity: 'error',
			code: 'INSUFFICIENT_MEMORY',
			message: 'host lacks 32GB RAM',
			path: 'instances[0]',
			resource_ref: 'instance/web',
			blocking: true,
			suggestion: 'reduce instance memory or pick a larger host'
		}
	]
};

test.describe('Architecture Designer — Phase 3 fleet check', () => {
	test('happy-path: refresh inventory shows valid status and zero findings', async ({ page }) => {
		const backend = new FakeBackend(HAPPY_FLEET);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'fleet-happy');
		await page.getByTestId('tab-fleet').click();
		await expect(page.getByTestId('fleet-check-panel')).toBeVisible();

		// Idle state copy is visible before the first refresh.
		await expect(page.getByTestId('fleet-empty')).toBeVisible();

		await page.getByTestId('fleet-refresh-button').click();

		// After the mocked response lands the status pill renders "Valid"
		// and no findings are listed.
		await expect(page.getByTestId('fleet-status-pill')).toHaveText(/valid/i);
		await expect(page.getByTestId('fleet-count-errors')).toHaveText(/0 errors/);
		await expect(page.getByTestId('fleet-deploy-blocked-banner')).toBeHidden();

		// Axe scan after refresh — same exclusions as the canvas suite.
		const results = await new AxeBuilder({ page })
			.disableRules(['color-contrast', 'region'])
			.include('[data-testid="fleet-check-panel"]')
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		const blocking = results.violations.filter(
			(v) => v.impact === 'serious' || v.impact === 'critical'
		);
		expect(blocking).toEqual([]);
	});

	test('deploy-blocked: an error finding renders the blocking banner and the finding', async ({
		page
	}) => {
		const backend = new FakeBackend(BLOCKED_FLEET);
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'fleet-blocked');
		await page.getByTestId('tab-fleet').click();
		await page.getByTestId('fleet-refresh-button').click();

		// Banner surfaces with the error count and the alert role.
		const banner = page.getByTestId('fleet-deploy-blocked-banner');
		await expect(banner).toBeVisible();
		await expect(banner).toHaveText(/1 fleet error/i);
		await expect(banner).toHaveAttribute('role', 'alert');

		// Status pill flips to "Invalid".
		await expect(page.getByTestId('fleet-status-pill')).toHaveText(/invalid/i);

		// The finding row is rendered (FindingItem reuse from Phase 1).
		await expect(page.getByTestId('finding-item').first()).toBeVisible();
		await expect(page.getByTestId('finding-code').first()).toHaveText(/INSUFFICIENT_MEMORY/);
		await expect(page.getByTestId('finding-message').first()).toContainText(
			/host lacks 32GB RAM/i
		);

		// Axe scan with errors present — banner still must not introduce
		// serious/critical violations.
		const results = await new AxeBuilder({ page })
			.disableRules(['color-contrast', 'region'])
			.include('[data-testid="fleet-check-panel"]')
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		const blocking = results.violations.filter(
			(v) => v.impact === 'serious' || v.impact === 'critical'
		);
		expect(blocking).toEqual([]);
	});
});
