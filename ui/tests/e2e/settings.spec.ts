import { test, expect } from '@playwright/test';
import { loginAsAdmin, mockApiResponse, mockSettings, mockHypervisorSettings } from './helpers';

test.describe('Settings', () => {
	test.beforeEach(async ({ page }) => {
		await loginAsAdmin(page);
		await mockApiResponse(page, '**/v1/settings', mockSettings);
	});

	test('settings page loads and shows infrastructure environment', async ({ page }) => {
		await page.goto('/settings');
		await expect(page.getByRole('heading', { name: 'Settings / Access' })).toBeVisible();
		await expect(page.getByText('Infrastructure Environment')).toBeVisible();
		await expect(page.getByText('v0.1.0').first()).toBeVisible();
	});

	test('hypervisor settings page loads with toggles', async ({ page }) => {
		await mockApiResponse(page, '**/v1/settings/hypervisor', mockHypervisorSettings);
		await page.goto('/settings/hypervisor');
		await expect(page.getByText('Fabric Infrastructure')).toBeVisible();
		await expect(page.getByText('Compute Fabric')).toBeVisible();
	});
});
