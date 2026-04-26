import { test, expect } from '@playwright/test';
import { loginAsAdmin, mockApiResponse, mockVms, mockImages, mockNetworks } from './helpers';

test.describe('VM Management', () => {
	test.beforeEach(async ({ page }) => {
		await loginAsAdmin(page);
		await mockApiResponse(page, '**/v1/vms', mockVms);
		await mockApiResponse(page, '**/v1/images', mockImages);
		await mockApiResponse(page, '**/v1/networks', mockNetworks);
		await page.goto('/vms');
	});

	test('renders VM list table with mocked workloads', async ({ page }) => {
		await expect(page.getByRole('link', { name: 'web-server' })).toBeVisible();
		await expect(page.getByRole('link', { name: 'db-server' })).toBeVisible();
		await expect(page.getByRole('button', { name: /deploy workload/i })).toBeVisible();
	});

	test('filters VM list by search query', async ({ page }) => {
		const searchInput = page.getByPlaceholder(/name or node/i);
		await searchInput.fill('web');
		await searchInput.press('Enter');
		await expect(page).toHaveURL(/query=web/);
		await expect(page.getByRole('link', { name: 'web-server' })).toBeVisible();
	});

	test('clears VM filters', async ({ page }) => {
		const searchInput = page.getByPlaceholder(/name or node/i);
		await searchInput.fill('web');
		await searchInput.press('Enter');
		await expect(page).toHaveURL(/query=web/);
		await page.getByRole('button', { name: /clear all/i }).click();
		await expect(page).not.toHaveURL(/query=web/);
	});

	test('opens create VM modal and validates name', async ({ page }) => {
		await page.getByRole('button', { name: /deploy workload/i }).click();
		await expect(page.getByText(/create vm - step 1 of 3/i)).toBeVisible();
		const nameInput = page.locator('#vm-name');
		await nameInput.fill('bad_name!');
		await nameInput.blur();
		await expect(page.getByText(/name must contain only lowercase/i)).toBeVisible();
	});

	test('create VM modal pre-selects first image and network', async ({ page }) => {
		await page.getByRole('button', { name: /deploy workload/i }).click();
		await expect(page.locator('#vm-image')).toHaveValue('img-1');
		await expect(page.locator('#vm-network')).toHaveValue('net-1');
	});
});
