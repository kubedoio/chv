import { test, expect } from '@playwright/test';
import { mockApiResponse } from './helpers';

test.describe('Authentication', () => {
	test('has expected login form', async ({ page }) => {
		await page.goto('/login');

		await expect(page.getByLabel(/operator identity/i)).toBeVisible();
		await expect(page.getByLabel(/access credential/i)).toBeVisible();
		await expect(page.getByRole('button', { name: /authenticate session/i })).toBeVisible();
	});

	test('shows error on invalid credentials', async ({ page }) => {
		await page.goto('/login');
		await mockApiResponse(page, '/v1/auth/login', { error: { message: 'Invalid credentials' } }, 401);

		await page.getByLabel(/operator identity/i).fill('admin');
		await page.getByLabel(/access credential/i).fill('wrong-password');
		await page.getByRole('button', { name: /authenticate session/i }).click();

		await expect(page.locator('.login-error')).toContainText('Invalid credentials');
	});

	test('successful login stores token and redirects to dashboard', async ({ page }) => {
		await page.goto('/login');
		await mockApiResponse(page, '/v1/auth/login', {
			token: 'mock-jwt-token-123',
			user: { username: 'admin', role: 'admin' }
		});

		await page.getByLabel(/operator identity/i).fill('admin');
		await page.getByLabel(/access credential/i).fill('correct-password');
		await page.getByRole('button', { name: /authenticate session/i }).click();

		await expect(page).toHaveURL('/');
		const token = await page.evaluate(() => localStorage.getItem('chv-api-token'));
		expect(token).toBe('mock-jwt-token-123');
	});

	test('token persistence allows direct access to dashboard', async ({ page }) => {
		await page.addInitScript(() => {
			localStorage.setItem('chv-api-token', 'existing-token');
		});
		await page.goto('/');
		await expect(page.getByText('Fleet Overview')).toBeVisible();
	});
});
