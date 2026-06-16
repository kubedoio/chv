import { test, expect, type Page, type Route } from '@playwright/test';
import { loginAsAdmin } from './helpers';

/**
 * Architecture Designer — Stage C starter topology surface.
 *
 * Stage C ships a Clone button + read-only banner on starter detail pages,
 * plus a populated dashboard on fresh boot once the controlplane has seeded
 * the six starters (see `docs/plans/2026-06-16-starter-topologies-and-auto-seed.md`).
 *
 * Detection rule (mirrored in the UI helper `$lib/architectures/starter`):
 *   isStarter = name.startsWith('starter-') AND owner_user_id === null
 *
 * The wire intentionally has NO `labels` field — see the plan §5 and the
 * `Architecture` type in `$lib/bff/architectures`. These tests only mock
 * the BFF endpoints actually exercised; the architecture-skeleton spec
 * already covers the broader surface.
 *
 * Clone shape: the BFF `createArchitecture` accepts `design_graph_json`
 * and `latest_yaml` directly (CreateArchitectureRequest), so the Stage C
 * clone is a single round-trip — no follow-up `updateArchitecture` is
 * required and these mocks reflect that.
 */

interface MockArch {
	id: string;
	name: string;
	display_name: string | null;
	description: string | null;
	environment: string | null;
	status: string;
	owner_user_id: string | null;
	last_validation_status: 'unknown' | 'passed' | 'failed' | null;
	last_fleet_check_status: 'unknown' | 'passed' | 'failed' | null;
	version_number: number;
	created_at: string;
	updated_at: string;
	archived_at: string | null;
}

function makeStarter(idx: number, slug: string, displayName: string): MockArch {
	const now = '2026-06-16T00:00:00Z';
	return {
		id: `arch-starter-${idx}`,
		name: `starter-${slug}`,
		display_name: displayName,
		description: `${displayName} starter topology`,
		environment: idx === 3 || idx === 4 || idx === 5 ? 'staging' : 'development',
		status: 'draft',
		owner_user_id: null,
		last_validation_status: null,
		last_fleet_check_status: null,
		version_number: 1,
		created_at: now,
		updated_at: now,
		archived_at: null
	};
}

const SIX_STARTERS: MockArch[] = [
	makeStarter(1, '01-single-vm', 'Single Linux Dev VM'),
	makeStarter(2, '02-lamp', 'LAMP / WordPress single-server'),
	makeStarter(3, '03-three-tier', 'Three-tier Web (Web / App / DB)'),
	makeStarter(4, '04-k8s-ha', 'Kubernetes HA (3+3, stacked etcd)'),
	makeStarter(5, '05-observability', 'Prometheus + Grafana observability stack'),
	makeStarter(6, '06-k3s-edge', 'K3s single-node edge')
];

interface MockRouteOpts {
	architectures: MockArch[];
	/** Optional override for `/get` so a single test can return a specific arch + body. */
	getOverrides?: Map<
		string,
		{ architecture: MockArch; design_graph_json: string | null; latest_yaml: string | null }
	>;
	/** Captures every body posted to /create so tests can assert payload fields. */
	createSpy?: { body: unknown }[];
	/** Architecture id returned by /create. Defaults to `arch-clone-1`. */
	createdId?: string;
}

async function installMocks(page: Page, opts: MockRouteOpts) {
	const json = (route: Route, status: number, body: unknown) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			body: JSON.stringify(body)
		});

	await page.route('**/v1/architectures/list', async (route) => {
		await json(route, 200, { architectures: opts.architectures });
	});

	await page.route('**/v1/architectures/get', async (route) => {
		const body = route.request().postDataJSON() as { id: string };
		const override = opts.getOverrides?.get(body.id);
		if (override) {
			await json(route, 200, override);
			return;
		}
		const arch = opts.architectures.find((a) => a.id === body.id);
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

	await page.route('**/v1/architectures/create', async (route) => {
		const body = route.request().postDataJSON();
		opts.createSpy?.push({ body });
		const id = opts.createdId ?? 'arch-clone-1';
		const now = '2026-06-16T00:00:00Z';
		const cloned: MockArch = {
			id,
			name: (body as { name: string }).name,
			display_name: (body as { display_name: string | null }).display_name ?? null,
			description: (body as { description: string | null }).description ?? null,
			environment: (body as { environment: string | null }).environment ?? null,
			status: 'draft',
			owner_user_id: 'u-test',
			last_validation_status: null,
			last_fleet_check_status: null,
			version_number: 1,
			created_at: now,
			updated_at: now,
			archived_at: null
		};
		// Also surface the cloned row on subsequent /get calls so the post-goto
		// detail page renders without a 404.
		opts.architectures.push(cloned);
		await json(route, 200, { architecture: cloned });
	});

	// Sidebar fetches so they don't 404 noisily.
	await page.route('**/v1/nodes', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
	await page.route('**/v1/vms', async (route) =>
		json(route, 200, { items: [], page: { page: 1, page_size: 50, total_items: 0 } })
	);
}

test.describe('Architecture Designer — Stage C starter topologies', () => {
	test.beforeEach(async ({ page }) => {
		await loginAsAdmin(page);
	});

	test('starter-dashboard-shows-six-cards', async ({ page }) => {
		await installMocks(page, { architectures: [...SIX_STARTERS] });

		await page.goto('/architectures');
		await expect(page.getByTestId('architectures-list')).toBeVisible();

		const cards = page.getByTestId('architecture-card');
		await expect(cards).toHaveCount(6);

		// Every card should render its display name; spot-check a couple to
		// catch silent regressions in the card title binding.
		await expect(page.getByText('Single Linux Dev VM').first()).toBeVisible();
		await expect(
			page.getByText('Kubernetes HA (3+3, stacked etcd)').first()
		).toBeVisible();
	});

	test('clone-starter-creates-new-architecture', async ({ page }) => {
		const starter = SIX_STARTERS[0]!;
		const createSpy: { body: unknown }[] = [];
		await installMocks(page, {
			architectures: [starter],
			getOverrides: new Map([
				[
					starter.id,
					{
						architecture: starter,
						// Non-null body so the clone request carries the deep-copy
						// fields (the real BFF creates the full row in one call).
						design_graph_json: '{"version":"1.0","nodes":[],"edges":[]}',
						latest_yaml: 'apiVersion: chv.kubedo.io/v1alpha1\nkind: CHVArchitecture\n'
					}
				]
			]),
			createSpy,
			createdId: 'arch-clone-1'
		});

		await page.goto(`/architectures/${starter.id}`);

		// Banner + button visible on a starter.
		await expect(page.getByTestId('starter-banner')).toBeVisible();
		const cloneButton = page.getByTestId('clone-starter');
		await expect(cloneButton).toBeVisible();
		await expect(cloneButton).toHaveText(/clone starter/i);

		await cloneButton.click();

		// Lands on the cloned architecture's detail page.
		await expect(page).toHaveURL(/\/architectures\/arch-clone-1$/);

		// Payload assertions: prefix dropped, suffix added, deep-copy fields
		// forwarded so the BFF can persist the clone in a single round-trip.
		expect(createSpy).toHaveLength(1);
		const body = createSpy[0]!.body as {
			name: string;
			display_name: string;
			description: string | null;
			environment: string | null;
			design_graph_json: string | null;
			latest_yaml: string | null;
		};
		expect(body.name).toMatch(/^01-single-vm-clone-[a-z0-9]+$/i);
		expect(body.display_name).toBe('Single Linux Dev VM (clone)');
		expect(body.description).toBe('Single Linux Dev VM starter topology');
		expect(body.environment).toBe('development');
		expect(body.design_graph_json).toContain('"version":"1.0"');
		expect(body.latest_yaml).toContain('CHVArchitecture');
	});

	test('starter-banner-not-shown-for-user-architectures', async ({ page }) => {
		const userArch: MockArch = {
			...SIX_STARTERS[0]!,
			id: 'arch-user-1',
			name: 'my-arch',
			owner_user_id: 'u-1'
		};
		await installMocks(page, { architectures: [userArch] });

		await page.goto(`/architectures/${userArch.id}`);
		await expect(page.getByTestId('architecture-detail-page')).toBeVisible();
		await expect(page.getByTestId('starter-banner')).toHaveCount(0);
		await expect(page.getByTestId('clone-starter')).toHaveCount(0);
	});
});
