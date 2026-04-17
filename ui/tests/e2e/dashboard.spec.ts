import { test, expect } from '@playwright/test';

test.describe('Dashboard View', () => {
	test.beforeEach(async ({ page }) => {
		// Insert fake token to bypass login page redirection
		await page.addInitScript(() => {
			window.localStorage.setItem('chv_token', 'fake-jwt-token');
		});

		await page.goto('/');
	});

	test('shows empty overview cards', async ({ page }) => {
		// Verify that we land on Datacenter overview and see expected stats
		await expect(page).toHaveTitle(/CHV Manager/);
		
		// We expect the text "Total VMs" to be visible
		await expect(page.getByText('Total VMs')).toBeVisible();
		
		// The value should be 0 based on our mock
		const vmsCardCount = page.locator('.flex-1:has-text("Total VMs")').locator('.text-\\[32px\\]');
		await expect(vmsCardCount).toHaveText('0');
	});
});
