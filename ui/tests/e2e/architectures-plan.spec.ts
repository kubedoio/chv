import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer Phase 4 — Layer 2 plan-review tab.
 *
 * Mirrors the structure of `architectures-fleet-check.spec.ts`: we mock the
 * BFF endpoints with `page.route(...)` so this suite does not depend on the
 * Rust BFF having compiled — only on the documented wire shape from
 * `docs/specs/architecture-designer/contracts/api-contract.md` and the Phase 4
 * plan in `docs/plans/`.
 *
 * Wire shape under test (Phase 4):
 *   - List   -> `{ architectures: [...] }`
 *   - Get    -> `{ architecture, design_graph_json, latest_yaml }`
 *   - plan   -> `PlanResult` (see ui/src/lib/bff/architectures.ts)
 */

type ArchStatus = 'draft' | 'applied' | 'archived';

type PlanAction = 'create' | 'update' | 'delete' | 'replace' | 'no_op';
type PlanRisk = 'low' | 'medium' | 'high' | 'destructive';
type PlanMode = 'apply' | 'destroy';
type PlanStatus =
	| 'draft'
	| 'failed_validation'
	| 'requires_confirmation'
	| 'ready_to_apply'
	| 'applying'
	| 'applied'
	| 'failed'
	| 'expired'
	| 'discarded';

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

interface PlanChange {
	action: PlanAction;
	resource_type: string;
	resource_name: string;
	resource_ref: string;
	description: string;
	risk: PlanRisk;
	requires_confirmation: boolean;
}

interface PlanSummary {
	create: number;
	update: number;
	delete: number;
	replace: number;
	no_op: number;
	warnings: number;
}

interface PlanResponse {
	plan_id: string;
	architecture_id: string;
	architecture_version: number;
	architecture_version_id: string;
	status: PlanStatus;
	mode: PlanMode;
	summary: PlanSummary;
	changes: PlanChange[];
	warnings: string[];
	expires_at: string;
	created_at: string;
}

class FakeBackend {
	architectures: MockArch[] = [];
	private idCounter = 0;
	planResponse: PlanResponse;

	constructor(initialPlan: PlanResponse) {
		this.planResponse = initialPlan;
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
		const body = route.request().postDataJSON() as { id: string };
		const arch = backend.get(body.id);
		if (!arch) {
			await json(route, 404, { message: 'not found', code: 'NOT_FOUND' });
			return;
		}
		await json(route, 200, { architecture: arch });
	});

	await page.route('**/v1/architectures/plan', async (route) => {
		await json(route, 200, backend.planResponse);
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

const NOW_ISO = () => new Date().toISOString();
const ISO_PLUS = (ms: number) => new Date(Date.now() + ms).toISOString();
const ISO_MINUS = (ms: number) => new Date(Date.now() - ms).toISOString();

const HAPPY_CHANGES: PlanChange[] = [
	{
		action: 'create',
		resource_type: 'network',
		resource_name: 'tenant-a',
		resource_ref: 'network/tenant-a',
		description: 'Create network tenant-a',
		risk: 'low',
		requires_confirmation: false
	},
	{
		action: 'create',
		resource_type: 'network',
		resource_name: 'tenant-b',
		resource_ref: 'network/tenant-b',
		description: 'Create network tenant-b',
		risk: 'low',
		requires_confirmation: false
	},
	{
		action: 'create',
		resource_type: 'instance',
		resource_name: 'web',
		resource_ref: 'instance/web',
		description: 'Create instance web',
		risk: 'low',
		requires_confirmation: false
	},
	{
		action: 'create',
		resource_type: 'instance',
		resource_name: 'db',
		resource_ref: 'instance/db',
		description: 'Create instance db',
		risk: 'medium',
		requires_confirmation: false
	},
	{
		action: 'create',
		resource_type: 'volume',
		resource_name: 'data',
		resource_ref: 'volume/data',
		description: 'Create volume data',
		risk: 'low',
		requires_confirmation: false
	}
];

const happyPlan = (): PlanResponse => ({
	plan_id: 'plan_01HX',
	architecture_id: 'arch-1',
	architecture_version: 1,
	architecture_version_id: 'ver_01HX',
	status: 'ready_to_apply',
	mode: 'apply',
	summary: { create: 5, update: 0, delete: 0, replace: 0, no_op: 0, warnings: 0 },
	changes: HAPPY_CHANGES,
	warnings: [],
	expires_at: ISO_PLUS(15 * 60 * 1000),
	created_at: NOW_ISO()
});

const expiredPlan = (): PlanResponse => ({
	plan_id: 'plan_01HZ',
	architecture_id: 'arch-1',
	architecture_version: 1,
	architecture_version_id: 'ver_01HZ',
	status: 'expired',
	mode: 'apply',
	summary: { create: 5, update: 0, delete: 0, replace: 0, no_op: 0, warnings: 0 },
	changes: HAPPY_CHANGES,
	warnings: [],
	expires_at: ISO_MINUS(60 * 60 * 1000),
	created_at: ISO_MINUS(75 * 60 * 1000)
});

const blockedPlan = (): PlanResponse => ({
	plan_id: 'plan_01HW',
	architecture_id: 'arch-1',
	architecture_version: 1,
	architecture_version_id: 'ver_01HW',
	status: 'failed_validation',
	mode: 'apply',
	summary: { create: 0, update: 0, delete: 0, replace: 0, no_op: 0, warnings: 0 },
	changes: [],
	warnings: ['IMAGE_NOT_FOUND: image ubuntu-22 missing from fleet'],
	expires_at: ISO_PLUS(15 * 60 * 1000),
	created_at: NOW_ISO()
});

test.describe('Architecture Designer — Phase 4 plan generation', () => {
	test('plan-happy: ready-to-apply plan renders summary, changes, and a live TTL', async ({
		page
	}) => {
		const backend = new FakeBackend(happyPlan());
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'plan-happy');
		await page.getByTestId('tab-plan').click();
		await expect(page.getByTestId('plan-review-panel')).toBeVisible();
		await expect(page.getByTestId('plan-empty')).toBeVisible();

		await page.getByTestId('plan-generate-button').click();

		// Summary chips reflect the mocked counts.
		await expect(page.getByTestId('plan-summary-create')).toHaveText(/create\s+5/i);
		await expect(page.getByTestId('plan-summary-update')).toHaveText(/update\s+0/i);

		// At least one change row renders.
		await expect(page.getByTestId('plan-change-row').first()).toBeVisible();
		const rowCount = await page.getByTestId('plan-change-row').count();
		expect(rowCount).toBeGreaterThanOrEqual(1);

		// TTL badge is visible and NOT in the expired state.
		await expect(page.getByTestId('plan-ttl-countdown')).toBeVisible();
		await expect(page.getByTestId('plan-ttl-expired')).toBeHidden();

		// Axe scan scoped to the panel — same exclusions as the canvas / fleet
		// suites so we don't fail on cross-cutting design-token issues.
		const results = await new AxeBuilder({ page })
			.disableRules(['color-contrast', 'region'])
			.include('[data-testid="plan-review-panel"]')
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		const blocking = results.violations.filter(
			(v) => v.impact === 'serious' || v.impact === 'critical'
		);
		expect(blocking).toEqual([]);
	});

	test('plan-expired: expired plan renders the expired badge and apply stays disabled', async ({
		page
	}) => {
		const backend = new FakeBackend(expiredPlan());
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'plan-expired');
		await page.getByTestId('tab-plan').click();
		await page.getByTestId('plan-generate-button').click();

		// Expired badge is visible (client-side TTL detection).
		await expect(page.getByTestId('plan-ttl-expired')).toBeVisible();
		await expect(page.getByTestId('plan-ttl-countdown')).toBeHidden();

		// Apply is disabled (Phase 4 always disables it pending Phase 5).
		await expect(page.getByTestId('plan-apply-button')).toBeDisabled();
	});

	test('plan-blocked: failed-validation plan renders blocked banner and keeps generate enabled for retry', async ({
		page
	}) => {
		const backend = new FakeBackend(blockedPlan());
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'plan-blocked');
		await page.getByTestId('tab-plan').click();
		await page.getByTestId('plan-generate-button').click();

		// Blocked banner surfaces with the alert role and the warning copy.
		const banner = page.getByTestId('plan-blocked-banner');
		await expect(banner).toBeVisible();
		await expect(banner).toHaveAttribute('role', 'alert');
		await expect(page.getByTestId('plan-blocked-warning').first()).toContainText(
			/IMAGE_NOT_FOUND/
		);

		// Generate button still enabled so the operator can retry after fixing
		// the underlying validation finding.
		await expect(page.getByTestId('plan-generate-button')).toBeEnabled();
	});
});
