import { test, expect } from '@playwright/test';
import { loginAsAdmin, mockApiResponse } from './helpers';

test.describe('Navigation & Auth', () => {
	test.beforeEach(async ({ page }) => {
		await loginAsAdmin(page);
		await mockApiResponse(page, '**/v1/overview', {
			vms_total: 0, nodes_total: 0, alerts: [], recent_tasks: []
		});
		await page.goto('/');
	});

	test('sidebar navigation links route to expected pages', async ({ page }) => {
		const nav = page.getByRole('navigation', { name: 'Primary' });
		const links = [
			{ label: 'Images', url: '/images' },
			{ label: 'Networks', url: '/networks' },
			{ label: 'Storage Pools', url: '/volumes' },
			{ label: 'Tasks', url: '/tasks' },
			{ label: 'Events', url: '/events' },
			{ label: 'Settings', url: '/settings' }
		];
		for (const link of links) {
			await nav.getByRole('link', { name: link.label }).first().click();
			await page.waitForURL(link.url);
		}
	});

	test('command palette opens with Ctrl+K keyboard shortcut', async ({ page }) => {
		await page.evaluate(() => {
			document.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }));
		});
		await expect(page.locator('.fixed.inset-0')).toBeVisible();
		await expect(page.getByPlaceholder(/type a command or search/i)).toBeVisible();
	});

	test('command palette opens via top bar click', async ({ page }) => {
		await page.getByRole('button', { name: /open command palette/i }).click();
		await expect(page.locator('.fixed.inset-0')).toBeVisible();
	});

	test('logout clears token and redirects to login', async ({ page }) => {
		await page.getByRole('button', { name: /sign out/i }).click();
		await expect(page).toHaveURL('/login');
		expect(await page.evaluate(() => localStorage.getItem('chv-api-token'))).toBeNull();
	});

	test('unauthenticated user is redirected to login from protected route', async ({ browser }) => {
		const context = await browser.newContext();
		const newPage = await context.newPage();
		await newPage.goto('/vms');
		await expect(newPage).toHaveURL('/login', { timeout: 10000 });
		await context.close();
	});
});
