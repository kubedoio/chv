import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer Phase 5 — apply / destroy / runs flow.
 *
 * Mirrors the structure of architectures-plan.spec.ts — we route the BFF
 * with `page.route(...)` so this suite does not depend on the Rust BFF
 * compiling. Wire shape under test (Phase 5):
 *   - POST /v1/architectures/apply       -> ApplyRunResult
 *   - POST /v1/architectures/destroy     -> ApplyRunResult
 *   - POST /v1/architectures/runs/list   -> { runs: ApplyRunDetail[] }
 *
 * Endpoint fallback note: the per-run page derives the run by id from the
 * `runs/list` response (no `runs/get` endpoint exists yet); the mocks
 * mirror that contract.
 */

type ArchStatus = 'draft' | 'applied' | 'archived';
type RunStatus = 'queued' | 'running' | 'succeeded' | 'partially_failed' | 'failed' | 'cancelled';
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

interface PlanResponse {
	plan_id: string;
	architecture_id: string;
	architecture_version: number;
	architecture_version_id: string;
	status: PlanStatus;
	mode: PlanMode;
	summary: {
		create: number;
		update: number;
		delete: number;
		replace: number;
		no_op: number;
		warnings: number;
	};
	changes: PlanChange[];
	warnings: string[];
	expires_at: string;
	created_at: string;
}

interface ApplyRunDetail {
	id: string;
	architecture_id: string;
	architecture_version_id: string;
	plan_id: string | null;
	task_id: string | null;
	status: RunStatus;
	started_at: string | null;
	finished_at: string | null;
	requested_by: string | null;
	result_json: string | null;
	error_message: string | null;
	created_at: string;
	updated_at: string;
}

interface ApplyRunResult {
	run_id: string;
	task_id: string | null;
	status: RunStatus;
	started_at: string | null;
	architecture_id: string;
	architecture_version_id: string;
	plan_id: string;
}

const NOW_ISO = () => new Date().toISOString();
const ISO_PLUS = (ms: number) => new Date(Date.now() + ms).toISOString();

class FakeBackend {
	architectures: MockArch[] = [];
	runs: ApplyRunDetail[] = [];
	private idCounter = 0;
	planResponse: PlanResponse;
	applyShouldFail: { status: number; code: string; message: string } | null = null;
	nextRunOverride: ApplyRunDetail | null = null;

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
		const now = NOW_ISO();
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

	queueRun(architectureId: string, planId: string): ApplyRunResult {
		const id = `run_${this.runs.length + 1}`;
		const now = NOW_ISO();
		const run: ApplyRunDetail =
			this.nextRunOverride ?? {
				id,
				architecture_id: architectureId,
				architecture_version_id: 'ver_01HX',
				plan_id: planId,
				task_id: `task_${this.runs.length + 1}`,
				status: 'queued',
				started_at: now,
				finished_at: null,
				requested_by: 'admin',
				result_json: null,
				error_message: null,
				created_at: now,
				updated_at: now
			};
		this.runs.unshift(run);
		this.nextRunOverride = null;
		return {
			run_id: run.id,
			task_id: run.task_id,
			status: run.status,
			started_at: run.started_at,
			architecture_id: run.architecture_id,
			architecture_version_id: run.architecture_version_id,
			plan_id: run.plan_id ?? planId
		};
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
		await json(route, 200, { ...backend.planResponse, mode: 'apply' });
	});

	await page.route('**/v1/architectures/destroy-plan', async (route) => {
		await json(route, 200, { ...backend.planResponse, mode: 'destroy' });
	});

	await page.route('**/v1/architectures/apply', async (route) => {
		if (backend.applyShouldFail) {
			await json(route, backend.applyShouldFail.status, {
				message: backend.applyShouldFail.message,
				code: backend.applyShouldFail.code
			});
			return;
		}
		const body = route.request().postDataJSON() as {
			id: string;
			plan_id: string;
		};
		const result = backend.queueRun(body.id, body.plan_id);
		await json(route, 200, result);
	});

	await page.route('**/v1/architectures/destroy', async (route) => {
		const body = route.request().postDataJSON() as {
			id: string;
			plan_id: string;
		};
		const result = backend.queueRun(body.id, body.plan_id);
		await json(route, 200, result);
	});

	await page.route('**/v1/architectures/runs/list', async (route) => {
		await json(route, 200, { runs: backend.runs });
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

const APPLY_CHANGES: PlanChange[] = [
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
	}
];

const DESTROY_CHANGES: PlanChange[] = [
	{
		action: 'delete',
		resource_type: 'instance',
		resource_name: 'web',
		resource_ref: 'instance/web',
		description: 'Delete instance web',
		risk: 'destructive',
		requires_confirmation: true
	}
];

const applyPlan = (): PlanResponse => ({
	plan_id: 'plan_01HX',
	architecture_id: 'arch-1',
	architecture_version: 1,
	architecture_version_id: 'ver_01HX',
	status: 'ready_to_apply',
	mode: 'apply',
	summary: { create: 2, update: 0, delete: 0, replace: 0, no_op: 0, warnings: 0 },
	changes: APPLY_CHANGES,
	warnings: [],
	expires_at: ISO_PLUS(15 * 60 * 1000),
	created_at: NOW_ISO()
});

const destroyPlan = (): PlanResponse => ({
	plan_id: 'plan_01HD',
	architecture_id: 'arch-1',
	architecture_version: 1,
	architecture_version_id: 'ver_01HX',
	status: 'requires_confirmation',
	mode: 'destroy',
	summary: { create: 0, update: 0, delete: 1, replace: 0, no_op: 0, warnings: 0 },
	changes: DESTROY_CHANGES,
	warnings: [],
	expires_at: ISO_PLUS(15 * 60 * 1000),
	created_at: NOW_ISO()
});

test.describe('Architecture Designer — Phase 5 apply', () => {
	test('apply-happy-path: generate plan → confirm dialog → land on runs/[run_id] with queued status', async ({
		page
	}) => {
		const backend = new FakeBackend(applyPlan());
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'apply-happy');

		await page.getByTestId('tab-plan').click();
		await page.getByTestId('plan-generate-button').click();

		// Apply button enabled after plan is ready.
		const applyBtn = page.getByTestId('plan-apply-button');
		await expect(applyBtn).toBeEnabled();
		await applyBtn.click();

		const dialog = page.getByTestId('apply-confirm-dialog');
		await expect(dialog).toBeVisible();

		// Apply plan has no destructive changes / warnings, so confirm
		// button is enabled immediately.
		const confirmBtn = page.getByTestId('apply-confirm-button');
		await expect(confirmBtn).toBeEnabled();

		// Axe scan scoped to the dialog.
		const axe = await new AxeBuilder({ page })
			.disableRules(['color-contrast', 'region'])
			.include('[data-testid="apply-confirm-dialog"]')
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		const blocking = axe.violations.filter(
			(v) => v.impact === 'serious' || v.impact === 'critical'
		);
		expect(blocking).toEqual([]);

		await confirmBtn.click();

		await expect(page).toHaveURL(/\/architectures\/arch-1\/runs\/run_1$/);

		// The runs detail page renders the queued (or running) status badge.
		const badge = page.getByTestId('run-status-badge');
		await expect(badge).toBeVisible();
		await expect(badge).toHaveAttribute('data-status', /^(queued|running)$/);
	});

	test('destructive-confirmation: destroy plan requires typed-name, wrong name disables submit', async ({
		page
	}) => {
		const backend = new FakeBackend(destroyPlan());
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		await createArchitectureAndOpen(page, 'destroy-it');

		await page.getByTestId('tab-plan').click();
		await page.getByTestId('plan-destroy-button').click();

		const applyBtn = page.getByTestId('plan-apply-button');
		await expect(applyBtn).toBeEnabled();
		await applyBtn.click();

		const dialog = page.getByTestId('apply-confirm-dialog');
		await expect(dialog).toBeVisible();

		const typedNameInput = page.getByTestId('apply-typed-name-input');
		await expect(typedNameInput).toBeVisible();

		const confirmBtn = page.getByTestId('apply-confirm-button');
		// Initially disabled — empty input.
		await expect(confirmBtn).toBeDisabled();

		// Wrong name keeps it disabled.
		await typedNameInput.fill('not-the-name');
		await expect(confirmBtn).toBeDisabled();

		// Correct name enables it. Architecture was created with name "destroy-it".
		await typedNameInput.fill('destroy-it');
		await expect(confirmBtn).toBeEnabled();

		await confirmBtn.click();
		await expect(page).toHaveURL(/\/architectures\/arch-1\/runs\/run_1$/);
	});

	test('partial-failure: terminal partially_failed run renders error banner and per-op error', async ({
		page
	}) => {
		const backend = new FakeBackend(applyPlan());
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);

		// Override the run that the apply call will create so it lands in
		// `partially_failed` immediately with a structured result_json.
		backend.nextRunOverride = {
			id: 'run_1',
			architecture_id: 'arch-1',
			architecture_version_id: 'ver_01HX',
			plan_id: 'plan_01HX',
			task_id: 'task_1',
			status: 'partially_failed',
			started_at: NOW_ISO(),
			finished_at: NOW_ISO(),
			requested_by: 'admin',
			result_json: JSON.stringify({
				operations: [
					{
						resource_ref: 'instance/web',
						action: 'create',
						status: 'succeeded',
						operation_id: 'op_1'
					},
					{
						resource_ref: 'instance/db',
						action: 'create',
						status: 'failed',
						error_message: 'host capacity exceeded',
						operation_id: 'op_2'
					}
				]
			}),
			error_message: '1 of 2 operations failed',
			created_at: NOW_ISO(),
			updated_at: NOW_ISO()
		};

		await createArchitectureAndOpen(page, 'partial-fail');

		await page.getByTestId('tab-plan').click();
		await page.getByTestId('plan-generate-button').click();
		await page.getByTestId('plan-apply-button').click();
		await page.getByTestId('apply-confirm-button').click();

		await expect(page).toHaveURL(/\/architectures\/arch-1\/runs\/run_1$/);

		// Run-level partially_failed badge + error banner.
		const badge = page.getByTestId('run-status-badge');
		await expect(badge).toHaveAttribute('data-status', 'partially_failed');

		const banner = page.getByTestId('run-error-banner');
		await expect(banner).toBeVisible();
		await expect(page.getByTestId('run-error-message')).toContainText(
			/1 of 2 operations failed/
		);

		// Per-op rows render and the failed op surfaces its error.
		const rows = page.getByTestId('operation-progress-row');
		await expect(rows).toHaveCount(2);
		const failedRow = rows.filter({ has: page.getByTestId('operation-progress-error') });
		await expect(failedRow).toContainText(/host capacity exceeded/);
	});
});
