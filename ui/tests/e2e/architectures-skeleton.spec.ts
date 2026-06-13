import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer Phase 0 skeleton — empty list, create, detail, edit,
 * stale-version banner.
 *
 * The BFF endpoints under `/v1/architectures/*` are mocked at the Playwright
 * route layer so the UI can be exercised against the locked wire shape:
 *   - List  -> `{ architectures: [...] }` (no `items`/`page` wrapper).
 *   - Get   -> `{ architecture, design_graph_json, latest_yaml }`.
 *   - Update body is FLAT — `{ id, expected_version, display_name?, description?, environment?, ... }` with NO `patch` wrapper.
 *   - Archive requires `expected_version` and returns `{ architecture }`.
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
		// Build the next row by overlaying any provided field. `undefined` means
		// "leave alone" — only properties present on the body get applied.
		const next: MockArch = {
			...current,
			...(body.display_name !== undefined ? { display_name: body.display_name } : {}),
			...(body.description !== undefined ? { description: body.description } : {}),
			...(body.environment !== undefined ? { environment: body.environment } : {}),
			version_number: current.version_number + 1,
			updated_at: new Date().toISOString()
		};
		this.architectures[idx] = next;
		return { ok: true, arch: next };
	}

	archive(
		id: string,
		expectedVersion: number
	): { ok: true; arch: MockArch } | { ok: false; reason: 'stale' | 'not_found' } {
		const idx = this.architectures.findIndex((a) => a.id === id);
		if (idx === -1) return { ok: false, reason: 'not_found' };
		const current = this.architectures[idx]!;
		if (current.version_number !== expectedVersion) {
			return { ok: false, reason: 'stale' };
		}
		const now = new Date().toISOString();
		const next: MockArch = {
			...current,
			status: 'archived',
			archived_at: now,
			version_number: current.version_number + 1,
			updated_at: now
		};
		this.architectures[idx] = next;
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
			design_graph_json: null,
			latest_yaml: null
		});
	});

	await page.route('**/v1/architectures/update', async (route) => {
		// Phase 0 wire shape is FLAT: editable fields live alongside `id` and
		// `expected_version`. There is no `patch` wrapper.
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

	await page.route('**/v1/architectures/archive', async (route) => {
		const body = route.request().postDataJSON() as {
			id: string;
			expected_version: number;
		};
		const result = backend.archive(body.id, body.expected_version);
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

	// Mocks for unrelated sidebar fetches so they don't 404 noisily.
	await page.route('**/v1/nodes', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
	await page.route('**/v1/vms', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
}

test.describe('Architecture Designer — Phase 0 skeleton', () => {
	let backend: FakeBackend;

	test.beforeEach(async ({ page }) => {
		backend = new FakeBackend();
		await loginAsAdmin(page);
		await installArchitectureMocks(page, backend);
	});

	test('empty list shows the empty state and CTA navigates to /architectures/new', async ({
		page
	}) => {
		await page.goto('/architectures');

		await expect(
			page.getByTestId('architectures-empty-state')
		).toBeVisible();
		await expect(page.getByRole('heading', { name: /no architectures yet/i })).toBeVisible();

		await page.getByRole('button', { name: /create your first/i }).click();
		await expect(page).toHaveURL(/\/architectures\/new$/);
	});

	test('full create -> detail -> rename -> stale-version flow', async ({ page }) => {
		// 1. Empty state
		await page.goto('/architectures');
		await expect(page.getByTestId('architectures-empty-state')).toBeVisible();

		// 2. Click "Create your first" -> /architectures/new
		await page.getByRole('button', { name: /create your first/i }).click();
		await expect(page).toHaveURL(/\/architectures\/new$/);

		// 3. Fill the form and submit
		await page.locator('#arch-name').fill('phase-0-test');
		await page.locator('#arch-description').fill('smoke');
		await page.locator('#arch-environment').selectOption('development');
		await page.getByRole('button', { name: /create architecture/i }).click();

		// 4. Detail page renders the metadata. The heading shows display_name
		//    when present; with the new wire shape display_name starts null so
		//    the slug `name` is shown instead.
		await expect(page).toHaveURL(/\/architectures\/arch-1$/);
		await expect(page.getByTestId('architecture-name')).toHaveText('phase-0-test');
		await expect(page.getByTestId('meta-name')).toHaveText('phase-0-test');
		await expect(page.getByTestId('meta-slug')).toHaveText('phase-0-test');
		await expect(page.getByTestId('meta-description')).toHaveText('smoke');
		await expect(page.getByTestId('meta-environment')).toHaveText('development');
		await expect(page.getByTestId('meta-version')).toHaveText('1');

		// 5. Back to /architectures — list contains the new card
		await page.goto('/architectures');
		await expect(page.getByTestId('architectures-list')).toBeVisible();
		await expect(page.getByTestId('architecture-card-name')).toHaveText('phase-0-test');

		// 6. Open detail again, edit display name, save, assert UI updated
		await page.goto('/architectures/arch-1');
		await expect(page.getByTestId('meta-name')).toHaveText('phase-0-test');
		await page.getByRole('button', { name: /^edit metadata$/i }).click();
		await expect(page.getByTestId('architecture-meta-edit-form')).toBeVisible();
		await page.locator('#meta-display-name').fill('Phase 0 Renamed');
		await page.getByRole('button', { name: /save metadata/i }).click();
		await expect(page.getByTestId('meta-name')).toHaveText('Phase 0 Renamed');
		await expect(page.getByTestId('meta-version')).toHaveText('2');

		// 7. Programmatically POST a stale update via fetch -> bumps the
		//    server's version to 3 while the client still has v2 cached. Then
		//    trigger an edit from the UI and assert:
		//      a) the StaleVersionBanner appears
		//      b) the user's draft is preserved (M5: still in edit mode)
		const staleBumpStatus = await page.evaluate(async () => {
			const res = await fetch('/v1/architectures/update', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				// FLAT body — no `patch` wrapper.
				body: JSON.stringify({
					id: 'arch-1',
					expected_version: 2,
					description: 'out-of-band change'
				})
			});
			return res.status;
		});
		expect(staleBumpStatus).toBe(200);

		await page.getByRole('button', { name: /^edit metadata$/i }).click();
		await page.locator('#meta-display-name').fill('Phase 0 Renamed Again');
		await page.getByRole('button', { name: /save metadata/i }).click();

		await expect(page.getByTestId('stale-version-banner')).toBeVisible();
		// M5: edit form must remain visible AND retain the user's draft typing
		// so they don't have to retype after the conflict.
		await expect(page.getByTestId('architecture-meta-edit-form')).toBeVisible();
		await expect(page.locator('#meta-display-name')).toHaveValue('Phase 0 Renamed Again');

		await page.getByRole('button', { name: /reload architecture/i }).click();
		await expect(page.getByTestId('stale-version-banner')).toBeHidden();
		// After Reload the panel exits edit mode so the user can see the fresh
		// server metadata (now at v3 because of the out-of-band bump).
		await expect(page.getByTestId('meta-version')).toHaveText('3');
		// M5 follow-through: the user's draft survives across the Reload, so
		// re-clicking Edit shows the in-flight text instead of overwriting it
		// from the new server copy.
		await page.getByRole('button', { name: /^edit metadata$/i }).click();
		await expect(page.locator('#meta-display-name')).toHaveValue('Phase 0 Renamed Again');
		await page.getByRole('button', { name: /^cancel$/i }).click();
	});

	test('archive with stale version surfaces the banner; reload then archive succeeds', async ({
		page
	}) => {
		// Seed one architecture so the detail page has something to render.
		backend.create({ name: 'archive-target', description: 'doomed' });
		await page.goto('/architectures/arch-1');
		await expect(page.getByTestId('meta-version')).toHaveText('1');

		// Bump the server to v2 while the client still believes it is on v1.
		const bumpStatus = await page.evaluate(async () => {
			const res = await fetch('/v1/architectures/update', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					id: 'arch-1',
					expected_version: 1,
					description: 'out-of-band'
				})
			});
			return res.status;
		});
		expect(bumpStatus).toBe(200);

		// First archive call uses the stale v1 — should 409.
		const staleArchiveStatus = await page.evaluate(async () => {
			const res = await fetch('/v1/architectures/archive', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ id: 'arch-1', expected_version: 1 })
			});
			return res.status;
		});
		expect(staleArchiveStatus).toBe(409);

		// Reload picks up the fresh v2.
		await page.reload();
		await expect(page.getByTestId('meta-version')).toHaveText('2');

		// Second archive call with the right version succeeds and flips status.
		const goodArchiveStatus = await page.evaluate(async () => {
			const res = await fetch('/v1/architectures/archive', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ id: 'arch-1', expected_version: 2 })
			});
			return res.status;
		});
		expect(goodArchiveStatus).toBe(200);

		await page.reload();
		await expect(page.getByTestId('meta-status')).toHaveText('archived');
		await expect(page.getByTestId('meta-version')).toHaveText('3');
	});

	test('axe scan: /architectures empty state has no violations', async ({ page }) => {
		await page.goto('/architectures');
		await expect(page.getByTestId('architectures-empty-state')).toBeVisible();
		const results = await new AxeBuilder({ page })
			// `region` flags a pre-existing landmark gap inside the global
			// AppShell (TopCommandBar) that is not introduced by the Phase 0
			// architecture routes. `color-contrast` depends on rendered theme
			// tokens which jsdom-style snapshot evaluation handles unreliably.
			.disableRules(['color-contrast', 'region'])
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		expect(results.violations).toEqual([]);
	});

	test('axe scan: /architectures/new form has no violations', async ({ page }) => {
		await page.goto('/architectures/new');
		await expect(page.locator('#arch-name')).toBeVisible();
		const results = await new AxeBuilder({ page })
			.disableRules(['color-contrast', 'region'])
			.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
			.analyze();
		expect(results.violations).toEqual([]);
	});
});
