# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: navigation.spec.ts >> Navigation & Auth >> command palette opens with Ctrl+K keyboard shortcut
- Location: tests/e2e/navigation.spec.ts:29:2

# Error details

```
Error: expect(locator).toBeVisible() failed

Locator: getByPlaceholder(/type a command or search/i)
Expected: visible
Timeout: 5000ms
Error: element(s) not found

Call log:
  - Expect "toBeVisible" with timeout 5000ms
  - waiting for getByPlaceholder(/type a command or search/i)

```

```yaml
- region "Notifications"
- link "Skip to content":
  - /url: "#shell-main"
- complementary:
  - navigation "Primary":
    - text: CellHV Control Plane
    - searchbox "Search fleet resources"
    - link "Fleet Overview":
      - /url: /
    - text: Infrastructure No hosts enrolled. Global
    - link "Images":
      - /url: /images
    - link "Networks":
      - /url: /networks
    - link "Storage Pools":
      - /url: /storage
    - link "Tasks":
      - /url: /tasks
    - link "Events":
      - /url: /events
    - link "Backups":
      - /url: /backup-jobs
    - link "Settings":
      - /url: /settings
    - button "Toggle theme"
    - link "Settings":
      - /url: /settings
    - button "Sign out"
- text: Control plane Overview
- button "Open command palette": Search commands or jump to a resource ⌘K
- main:
  - article:
    - text: Empty Fleet
    - paragraph: No clusters or nodes are currently indexed.
    - paragraph: Enroll infrastructure to see real-time topology.
```

# Test source

```ts
  1  | import { test, expect } from '@playwright/test';
  2  | import { loginAsAdmin, mockApiResponse } from './helpers';
  3  |
  4  | test.describe('Navigation & Auth', () => {
  5  | 	test.beforeEach(async ({ page }) => {
  6  | 		await loginAsAdmin(page);
  7  | 		await mockApiResponse(page, '**/v1/overview', {
  8  | 			vms_total: 0, nodes_total: 0, alerts: [], recent_tasks: []
  9  | 		});
  10 | 		await page.goto('/');
  11 | 	});
  12 |
  13 | 	test('sidebar navigation links route to expected pages', async ({ page }) => {
  14 | 		const nav = page.getByRole('navigation', { name: 'Primary' });
  15 | 		const links = [
  16 | 			{ label: 'Images', url: '/images' },
  17 | 			{ label: 'Networks', url: '/networks' },
  18 | 			{ label: 'Storage Pools', url: '/volumes' },
  19 | 			{ label: 'Tasks', url: '/tasks' },
  20 | 			{ label: 'Events', url: '/events' },
  21 | 			{ label: 'Settings', url: '/settings' }
  22 | 		];
  23 | 		for (const link of links) {
  24 | 			await nav.getByRole('link', { name: link.label }).first().click();
  25 | 			await page.waitForURL(link.url);
  26 | 		}
  27 | 	});
  28 |
  29 | 	test('command palette opens with Ctrl+K keyboard shortcut', async ({ page }) => {
  30 | 		await page.evaluate(() => {
  31 | 			document.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }));
  32 | 		});
> 33 | 		await expect(page.getByPlaceholder(/type a command or search/i)).toBeVisible(); // wait for search input
     |                                                                    ^ Error: expect(locator).toBeVisible() failed
  34 | 			});
  35 |
  36 | 	test('command palette opens via top bar click', async ({ page }) => {
  37 | 		await page.getByRole('button', { name: /open command palette/i }).click();
  38 | 		await expect(page.locator('.fixed.inset-0')).toBeVisible();
  39 | 	});
  40 |
  41 | 	test('logout clears token and redirects to login', async ({ page }) => {
  42 | 		await page.getByRole('button', { name: /sign out/i }).click();
  43 | 		await expect(page).toHaveURL('/login');
  44 | 		expect(await page.evaluate(() => localStorage.getItem('chv-api-token'))).toBeNull();
  45 | 	});
  46 |
  47 | 	test('unauthenticated user is redirected to login from protected route', async ({ browser }) => {
  48 | 		const context = await browser.newContext();
  49 | 		const newPage = await context.newPage();
  50 | 		await newPage.goto('/vms');
  51 | 		await expect(newPage).toHaveURL('/login', { timeout: 10000 });
  52 | 		await context.close();
  53 | 	});
  54 | });
  55 |
```