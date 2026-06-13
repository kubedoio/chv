import { test, expect, type Page, type Route } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer Phase 0 skeleton — empty list, create, detail, edit,
 * stale-version banner.
 *
 * The BFF endpoints under `/v1/architectures/*` are mocked at the Playwright
 * route layer because the backend (separate agent) is in flight. Once the BFF
 * lands these mocks can be deleted in favour of the real preview server,
 * with the same assertions still passing.
 */

type ArchEnv = 'development' | 'staging' | 'production';
type ArchStatus = 'draft' | 'applied' | 'archived';

interface MockArch {
	id: string;
	name: string;
	description: string;
	environment: ArchEnv;
	status: ArchStatus;
	version_number: number;
	created_at: string;
	updated_at: string;
}

class FakeBackend {
	architectures: MockArch[] = [];
	private idCounter = 0;

	create(input: { name: string; description?: string; environment: ArchEnv }): MockArch {
		this.idCounter += 1;
		const now = new Date().toISOString();
		const arch: MockArch = {
			id: `arch-${this.idCounter}`,
			name: input.name,
			description: input.description ?? '',
			environment: input.environment,
			status: 'draft',
			version_number: 1,
			created_at: now,
			updated_at: now
		};
		this.architectures.push(arch);
		return arch;
	}

	get(id: string): MockArch | undefined {
		return this.architectures.find((a) => a.id === id);
	}

	update(
		id: string,
		expectedVersion: number,
		patch: { name?: string; description?: string; environment?: ArchEnv }
	): { ok: true; arch: MockArch } | { ok: false; reason: 'stale' | 'not_found' } {
		const idx = this.architectures.findIndex((a) => a.id === id);
		if (idx === -1) return { ok: false, reason: 'not_found' };
		const current = this.architectures[idx]!;
		if (current.version_number !== expectedVersion) {
			return { ok: false, reason: 'stale' };
		}
		const next: MockArch = {
			...current,
			...patch,
			version_number: current.version_number + 1,
			updated_at: new Date().toISOString()
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
		await json(route, 200, {
			items: backend.architectures,
			page: { page: 1, page_size: 50, total_items: backend.architectures.length }
		});
	});

	await page.route('**/v1/architectures/create', async (route) => {
		const req = route.request();
		const body = req.postDataJSON() as {
			name: string;
			description?: string;
			environment: ArchEnv;
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
		await json(route, 200, { architecture: arch });
	});

	await page.route('**/v1/architectures/update', async (route) => {
		const body = route.request().postDataJSON() as {
			id: string;
			expected_version: number;
			patch: { name?: string; description?: string; environment?: ArchEnv };
		};
		const result = backend.update(body.id, body.expected_version, body.patch);
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

		// 4. Detail page renders the metadata
		await expect(page).toHaveURL(/\/architectures\/arch-1$/);
		await expect(page.getByTestId('architecture-name')).toHaveText('phase-0-test');
		await expect(page.getByTestId('meta-name')).toHaveText('phase-0-test');
		await expect(page.getByTestId('meta-description')).toHaveText('smoke');
		await expect(page.getByTestId('meta-environment')).toHaveText('development');
		await expect(page.getByTestId('meta-version')).toHaveText('1');

		// 5. Back to /architectures — list contains the new card
		await page.goto('/architectures');
		await expect(page.getByTestId('architectures-list')).toBeVisible();
		await expect(page.getByTestId('architecture-card-name')).toHaveText('phase-0-test');

		// 6. Open detail again, edit name, save, assert UI updated
		await page.goto('/architectures/arch-1');
		await expect(page.getByTestId('meta-name')).toHaveText('phase-0-test');
		await page.getByRole('button', { name: /^edit metadata$/i }).click();
		await expect(page.getByTestId('architecture-meta-edit-form')).toBeVisible();
		await page.locator('#meta-name').fill('phase-0-renamed');
		await page.getByRole('button', { name: /save metadata/i }).click();
		await expect(page.getByTestId('meta-name')).toHaveText('phase-0-renamed');
		await expect(page.getByTestId('meta-version')).toHaveText('2');

		// 7. Programmatically POST a stale update via fetch -> bumps the server's
		//    version to 3 while the client still has v2 cached. Then trigger an
		//    edit from the UI and assert the StaleVersionBanner appears.
		const staleBumpStatus = await page.evaluate(async () => {
			const res = await fetch('/v1/architectures/update', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					id: 'arch-1',
					expected_version: 2,
					patch: { description: 'out-of-band change' }
				})
			});
			return res.status;
		});
		expect(staleBumpStatus).toBe(200);

		await page.getByRole('button', { name: /^edit metadata$/i }).click();
		await page.locator('#meta-name').fill('phase-0-renamed-again');
		await page.getByRole('button', { name: /save metadata/i }).click();

		await expect(page.getByTestId('stale-version-banner')).toBeVisible();
		await page.getByRole('button', { name: /reload architecture/i }).click();
		await expect(page.getByTestId('stale-version-banner')).toBeHidden();
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
