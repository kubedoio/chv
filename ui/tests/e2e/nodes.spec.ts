import { test, expect } from '@playwright/test';
import { loginAsAdmin, mockApiResponse, mockNodes, mockNodeDetail } from './helpers';

test.describe('Node Management', () => {
	test.beforeEach(async ({ page }) => {
		await loginAsAdmin(page);
		await mockApiResponse(page, '**/v1/nodes', mockNodes);
		await page.goto('/nodes');
	});

	test('renders node list table with mocked hosts', async ({ page }) => {
		await expect(page.getByRole('link', { name: 'hv-01' }).first()).toBeVisible();
		await expect(page.getByRole('button', { name: /enroll node/i })).toBeVisible();
	});

	test('navigates to node detail page', async ({ page }) => {
		await mockApiResponse(page, '**/v1/nodes/get', mockNodeDetail);
		await page.getByRole('link', { name: 'hv-01' }).first().click();
		await expect(page).toHaveURL(/nodes\/node-1/);
		await expect(page.getByText('Compute Posture')).toBeVisible();
	});

	test('filters node list by search query', async ({ page }) => {
		const searchInput = page.getByPlaceholder(/filter by node name or cluster/i);
		await searchInput.fill('hv-01');
		await searchInput.press('Enter');
		await expect(page.getByRole('link', { name: 'hv-01' }).first()).toBeVisible();
	});
});
