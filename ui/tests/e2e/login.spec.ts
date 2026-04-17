import { test, expect } from '@playwright/test';

test.describe('Authentication', () => {
	// Let's test the login page elements
	test('has expected login form', async ({ page }) => {
		await page.goto('/login');

		// Check the title
		await expect(page).toHaveTitle(/Login/);

		// Verify form elements exist
		const usernameInput = page.getByLabel(/username*/i);
		await expect(usernameInput).toBeVisible();

		const passwordInput = page.getByLabel(/password*/i);
		await expect(passwordInput).toBeVisible();

		const loginButton = page.getByRole('button', { name: /sign in/i });
		await expect(loginButton).toBeVisible();
	});

	test('shows error on invalid credentials', async ({ page }) => {
		await page.goto('/login');

		await page.getByLabel(/username*/i).fill('admin');
		await page.getByLabel(/password*/i).fill('wrong-password');
		await page.getByRole('button', { name: /sign in/i }).click();

		// Check for error boundary or toast
		// The app shows errors in a toast or form error block.
		await expect(page.getByText('Invalid credentials')).toBeVisible();
	});
});
