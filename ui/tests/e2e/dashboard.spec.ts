import { test, expect } from '@playwright/test';
import { loginAsAdmin, setupCommonMocks, navigateClientSide } from './helpers';

test.describe('Dashboard View', () => {
	test.beforeEach(async ({ page }) => {
		await loginAsAdmin(page);
		await setupCommonMocks(page);
		await page.goto('/');
	});

	test('shows fleet overview shell', async ({ page }) => {
		await expect(page.getByText('Fleet Overview')).toBeVisible();
	});

	test('displays overview metrics after client load', async ({ page }) => {
		// Navigate away and back to trigger client-side universal load
		await navigateClientSide(page, '/vms');
		await navigateClientSide(page, '/');
		await expect(page.getByText('Managed Nodes')).toBeVisible();
		await expect(page.getByText('Running Workloads')).toBeVisible();
	});
});
