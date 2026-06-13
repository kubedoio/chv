import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer Phase 2 — Svelte Flow canvas, palette, inspector.
 *
 * The full canvas flow is stateful (drag-add nodes, draw edges, persist,
 * reload), so this suite is wrapped in `test.describe.serial(...)`. The
 * tests share a single FakeBackend instance per run via beforeEach.
 *
 * Precondition: `playwright.config.ts` exports
 * `webServer.env.PUBLIC_ARCHITECTURE_DESIGNER_CANVAS = '1'` so the canvas
 * mounts. Without it the page renders the Phase-1 placeholder and these
 * tests will fail at the first canvas assertion.
 *
 * Wire-shape parity with `architectures-skeleton.spec.ts`:
 *   - List  -> `{ architectures: [...] }`
 *   - Get   -> `{ architecture, design_graph_json, latest_yaml }`
 *   - Update body is FLAT (`{ id, expected_version, ... }`) — no `patch` wrapper.
 *   - Validate echoes `findings: []` (empty by default for a freshly created
 *     architecture) and a `valid` status pill so the per-node badges stay gray.
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

interface UpdateBody {
	id: string;
	expected_version: number;
	display_name?: string | null;
	description?: string | null;
	environment?: string | null;
	design_graph_json?: string | null;
	latest_yaml?: string | null;
	latest_version_id?: string | null;
}

class FakeBackend {
	architectures: MockArch[] = [];
	designGraphJson = new Map<string, string | null>();
	latestYaml = new Map<string, string | null>();
	private idCounter = 0;

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
		this.designGraphJson.set(arch.id, null);
		this.latestYaml.set(arch.id, null);
		return arch;
	}

	get(id: string): MockArch | undefined {
		return this.architectures.find((a) => a.id === id);
	}

	update(
		body: UpdateBody
	): { ok: true; arch: MockArch } | { ok: false; reason: 'stale' | 'not_found' } {
		const idx = this.architectures.findIndex((a) => a.id === body.id);
		if (idx === -1) return { ok: false, reason: 'not_found' };
		const current = this.architectures[idx]!;
		if (current.version_number !== body.expected_version) {
			return { ok: false, reason: 'stale' };
		}
		const next: MockArch = {
			...current,
			...(body.display_name !== undefined ? { display_name: body.display_name } : {}),
			...(body.description !== undefined ? { description: body.description } : {}),
			...(body.environment !== undefined ? { environment: body.environment } : {}),
			version_number: current.version_number + 1,
			updated_at: new Date().toISOString()
		};
		this.architectures[idx] = next;
		if (body.design_graph_json !== undefined) {
			this.designGraphJson.set(body.id, body.design_graph_json);
		}
		if (body.latest_yaml !== undefined) {
			this.latestYaml.set(body.id, body.latest_yaml);
		}
		return { ok: true, arch: next };
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
			design_graph_json: backend.designGraphJson.get(body.id) ?? null,
			latest_yaml: backend.latestYaml.get(body.id) ?? null
		});
	});

	await page.route('**/v1/architectures/update', async (route) => {
		const body = route.request().postDataJSON() as UpdateBody;
		const result = backend.update(body);
		if (!result.ok && result.reason === 'stale') {
			await json(route, 409, {
				message: 'Stale architecture version',
				code: 'STALE_VERSION'
			});
			return;
		}
		if (!result.ok) {
			await json(route, 404, { message: 'not found', code: 'NOT_FOUND' });
			return;
		}
		await json(route, 200, { architecture: result.arch });
	});

	await page.route('**/v1/architectures/validate', async (route) => {
		// Phase-2 canvas suite does not exercise validation finding rendering;
		// returning an empty result keeps the per-node badges gray and avoids
		// coupling the canvas tests to validator output shape.
		await json(route, 200, {
			status: 'valid',
			summary: { errors: 0, warnings: 0, info: 0 },
			findings: []
		});
	});

	// Mocks for unrelated sidebar fetches so they don't 404 noisily.
	await page.route('**/v1/nodes', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
	await page.route('**/v1/vms', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
}

/**
 * Drag a palette tile onto the canvas drop zone. Svelte Flow handles the
 * native HTML5 drag/drop events; Playwright's `dragTo` triggers them in the
 * right order. We use the data-testid the palette + canvas components are
 * expected to expose:
 *   - palette tile: `palette-tile-<kind>`
 *   - canvas drop zone: `canvas-dropzone`
 */
async function dragPaletteTile(page: Page, kind: string) {
	const tile = page.getByTestId(`palette-tile-${kind}`);
	const dropzone = page.getByTestId('canvas-dropzone');
	await tile.dragTo(dropzone);
}

async function createArchitectureAndOpen(page: Page, name: string) {
	await page.goto('/architectures/new');
	await page.locator('#arch-name').fill(name);
	await page.locator('#arch-environment').selectOption('development');
	await page.getByRole('button', { name: /create architecture/i }).click();
	await expect(page).toHaveURL(/\/architectures\/arch-1$/);
	await expect(page.getByTestId('canvas-shell')).toBeVisible();
}

test.describe.serial('Architecture Designer — Phase 2 canvas', () => {
	let backend: FakeBackend;

	test.beforeEach(async ({ page }) => {
		backend = new FakeBackend();
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);
	});

	test('drag-add-host: dragging the host palette tile creates a host node', async ({ page }) => {
		await createArchitectureAndOpen(page, 'canvas-host');
		await dragPaletteTile(page, 'host');
		// Node containers expose data-testid="canvas-node-<kind>" with a child
		// element rendering the kind label.
		await expect(page.getByTestId('canvas-node-host').first()).toBeVisible();
		await expect(page.getByTestId('canvas-node-host').first()).toContainText(/host/i);
	});

	test('drag-add-instance: dragging the instance palette tile creates an instance node', async ({
		page
	}) => {
		await createArchitectureAndOpen(page, 'canvas-instance');
		await dragPaletteTile(page, 'instance');
		await expect(page.getByTestId('canvas-node-instance').first()).toBeVisible();
		await expect(page.getByTestId('canvas-node-instance').first()).toContainText(/instance/i);
	});

	test('draw placed_on edge from instance to host', async ({ page }) => {
		await createArchitectureAndOpen(page, 'canvas-edge');
		await dragPaletteTile(page, 'host');
		await dragPaletteTile(page, 'instance');

		// Drag from the instance source handle to the host target handle. The
		// node components are expected to render Svelte Flow handles with
		// data-testid="node-handle-source-<id>" / "node-handle-target-<id>".
		const instanceSource = page
			.locator('[data-testid^="node-handle-source-node-instance-"]')
			.first();
		const hostTarget = page.locator('[data-testid^="node-handle-target-node-host-"]').first();
		await instanceSource.dragTo(hostTarget);

		// Svelte Flow renders edges with class `svelte-flow__edge` (current
		// xyflow convention). Asserting one such element exists is sufficient.
		await expect(page.locator('.svelte-flow__edge').first()).toBeVisible();
	});

	test('edge-reject: drawing a forbidden edge surfaces a toast', async ({ page }) => {
		await createArchitectureAndOpen(page, 'canvas-reject');
		await dragPaletteTile(page, 'instance');
		await dragPaletteTile(page, 'network');

		// instance -> network IS allowed (attached_to_network), so to exercise
		// the rejection path we wire a forbidden edge instead: network ->
		// instance (no rule). The handle naming convention is symmetric so
		// either direction can be attempted.
		const networkSource = page
			.locator('[data-testid^="node-handle-source-node-network-"]')
			.first();
		const instanceTarget = page
			.locator('[data-testid^="node-handle-target-node-instance-"]')
			.first();
		await networkSource.dragTo(instanceTarget);

		// Toasts use the global toast store; the rejection message contains
		// "not allowed" per `architecture-canvas-store.svelte.ts:addEdge`.
		await expect(page.getByText(/not allowed/i)).toBeVisible();
	});

	test('persist + reload: saved nodes survive a page reload', async ({ page }) => {
		await createArchitectureAndOpen(page, 'canvas-persist');
		await dragPaletteTile(page, 'host');
		await expect(page.getByTestId('canvas-node-host').first()).toBeVisible();

		// Dirty indicator should be visible after a node is added.
		await expect(page.getByTestId('canvas-dirty-indicator')).toBeVisible();

		await page.getByTestId('canvas-save-button').click();
		// Once persist resolves the indicator clears (see handleCanvasSave).
		await expect(page.getByTestId('canvas-dirty-indicator')).toBeHidden();

		await page.reload();
		await expect(page.getByTestId('canvas-shell')).toBeVisible();
		await expect(page.getByTestId('canvas-node-host').first()).toBeVisible();
	});

	test('axe scan: canvas shell has no serious or critical violations', async ({ page }) => {
		await createArchitectureAndOpen(page, 'canvas-a11y');
		await dragPaletteTile(page, 'host');
		await expect(page.getByTestId('canvas-node-host').first()).toBeVisible();

		const results = await new AxeBuilder({ page })
			// `region` flags a pre-existing landmark gap inside the global
			// AppShell — not introduced by Phase-2 canvas. `color-contrast`
			// depends on rendered theme tokens which axe-core sometimes
			// evaluates inconsistently against CSS-variable themes.
			.disableRules(['color-contrast', 'region'])
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();

		// Permit moderate / minor violations from the embedded Svelte Flow
		// chrome (its minimap and controls emit a few aria-warnings the host
		// app cannot fix). Anything `serious` or `critical` is a hard fail.
		const blocking = results.violations.filter(
			(v) => v.impact === 'serious' || v.impact === 'critical'
		);
		expect(blocking).toEqual([]);
	});
});
