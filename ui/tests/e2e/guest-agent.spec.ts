import { test, expect } from '@playwright/test';
import { loginAsAdmin, mockApiResponse } from './helpers';

/**
 * Guest monitoring agent surface (ADR-026, campaign #602 G3).
 *
 * Wire shape under test (agent_admin.rs):
 *   - POST /v1/monitoring/agents          -> inventory (state vocabulary)
 *   - POST /v1/monitoring/agents/claim    -> one-time claim token
 *   - POST /v1/monitoring/agents/revoke   -> { state: "revoked" }
 *
 * The card must render every state honestly — including the
 * identity_conflict extension — and the enrollment dialog must show
 * the claim token exactly once.
 */

const VM_DETAIL = {
	summary: {
		vm_id: 'vm-1',
		name: 'web-server',
		node_id: 'node-1',
		power_state: 'running',
		health: 'healthy',
		cpu: '2',
		memory: '4 GB',
		attached_volumes: [],
		attached_nics: [],
		recent_tasks: [],
		snapshot_count: 0
	}
};

function agentRow(overrides: Record<string, unknown> = {}) {
	return {
		agent_id: 'agent-1',
		vm_id: 'vm-1',
		state: 'active',
		install_id: 'install-1',
		credential_epoch: 3,
		credential_expires_at_ms: Date.now() + 21 * 86_400_000,
		rotation_pending: false,
		identity_conflict: false,
		conflict_reason: null,
		enrolled_at_ms: Date.now() - 86_400_000,
		last_seen_at_ms: Date.now() - 5_000,
		last_seen_age_seconds: 5,
		os: { name: 'Ubuntu', version: '24.04', kernel_release: '6.8.0-42-generic' },
		...overrides
	};
}

test.beforeEach(async ({ page }) => {
	await loginAsAdmin(page);
	await mockApiResponse(page, '**/v1/vms/get', VM_DETAIL);
	await page.goto('/vms/vm-1?tab=metrics');
	// The tab strip must be settled before the card asserts run.
	await expect(page.getByRole('heading', { name: /web-server/i })).toBeVisible();
});

test('renders an active agent with lifecycle actions', async ({ page }) => {
	await mockApiResponse(page, '**/v1/monitoring/agents', {
		schema_version: 1,
		agents: [agentRow()],
		generated_at_ms: Date.now(),
		truncated: false
	});
	await page.reload();

	const card = page.locator('section', { hasText: 'Guest monitoring agent' });
	await expect(card.getByText('Active')).toBeVisible();
	await expect(card.getByText(/Ubuntu 24\.04/)).toBeVisible();
	await expect(card.getByText(/6\.8\.0-42-generic/)).toBeVisible();
	await expect(card.getByText(/epoch 3/)).toBeVisible();
	await expect(card.getByRole('button', { name: 'Force rotation' })).toBeVisible();
	await expect(card.getByRole('button', { name: 'Revoke' })).toBeVisible();
	// A healthy agent offers no enrollment shortcut.
	await expect(card.getByRole('button', { name: 'Enroll agent' })).toHaveCount(0);
});

test('states the unenrolled case honestly and offers enrollment', async ({ page }) => {
	await mockApiResponse(page, '**/v1/monitoring/agents', {
		schema_version: 1,
		agents: [],
		generated_at_ms: Date.now(),
		truncated: false
	});
	await page.reload();

	const card = page.locator('section', { hasText: 'Guest monitoring agent' });
	await expect(card.getByText('Not enrolled')).toBeVisible();
	await expect(card.getByText(/fully usable without/i)).toBeVisible();
	await expect(card.getByRole('button', { name: 'Enroll agent' })).toBeVisible();
});

test('renders the enrolling state for an unredeemed claim (synthesized entry)', async ({ page }) => {
	// The backend synthesizes this entry from the live claim row: no
	// agent identity yet, so the identity fields are null.
	await mockApiResponse(page, '**/v1/monitoring/agents', {
		schema_version: 1,
		agents: [
			agentRow({
				agent_id: null,
				state: 'enrolling',
				install_id: null,
				credential_epoch: null,
				credential_expires_at_ms: null,
				enrolled_at_ms: null,
				last_seen_at_ms: null,
				last_seen_age_seconds: null,
				os: { name: null, version: null, kernel_release: null },
				claim_expires_at_ms: Date.now() + 9 * 60_000,
				claim_issued_by: 'admin'
			})
		],
		generated_at_ms: Date.now(),
		truncated: false
	});
	await page.reload();

	const card = page.locator('section', { hasText: 'Guest monitoring agent' });
	await expect(card.getByText('Enrolling')).toBeVisible();
	await expect(card.getByText(/waiting for redemption/i)).toBeVisible();
	await expect(card.getByText(/Claim expires in/i)).toBeVisible();
	// A claim-only entry offers re-issuance, never lifecycle actions on
	// an agent that does not exist yet.
	await expect(card.getByRole('button', { name: 'Re-issue claim' })).toBeVisible();
	await expect(card.getByRole('button', { name: 'Revoke' })).toHaveCount(0);
	await expect(card.getByRole('button', { name: 'Force rotation' })).toHaveCount(0);
});

test('renders the identity_conflict extension with a reset action', async ({ page }) => {
	await mockApiResponse(page, '**/v1/monitoring/agents', {
		schema_version: 1,
		agents: [
			agentRow({
				state: 'identity_conflict',
				identity_conflict: true,
				conflict_reason: 'install mismatch: cloned image suspected'
			})
		],
		generated_at_ms: Date.now(),
		truncated: false
	});
	await page.reload();

	const card = page.locator('section', { hasText: 'Guest monitoring agent' });
	await expect(card.getByText('Identity conflict')).toBeVisible();
	await expect(card.getByText(/cloned image/i)).toBeVisible();
	await expect(card.getByRole('button', { name: 'Clear conflict' })).toBeVisible();
	await expect(card.getByRole('button', { name: 'Revoke' })).toBeVisible();
});

test('enrollment dialog shows the one-time claim token', async ({ page }) => {
	await mockApiResponse(page, '**/v1/monitoring/agents', {
		schema_version: 1,
		agents: [],
		generated_at_ms: Date.now(),
		truncated: false
	});
	await mockApiResponse(page, '**/v1/monitoring/agents/claim', {
		schema_version: 1,
		vm_id: 'vm-1',
		claim_token: 'chvm_e2e_one_time_claim_token',
		expires_at_ms: Date.now() + 600_000,
		server_url: 'https://manager.example:8443',
		ca_fingerprint: 'aa:bb'
	});
	await page.reload();

	await page
		.locator('section', { hasText: 'Guest monitoring agent' })
		.getByRole('button', { name: 'Enroll agent' })
		.click();

	const dialog = page.locator('[role="dialog"]');
	await expect(dialog.getByText('Enroll guest monitoring agent')).toBeVisible();
	await expect(dialog.getByText('chvm_e2e_one_time_claim_token')).toBeVisible();
	await expect(dialog.getByText(/shown once, never again/i)).toBeVisible();
	await expect(dialog.getByText(/manager\.example:8443/)).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'Copy token' })).toBeVisible();

	await dialog.getByRole('button', { name: 'Done' }).click();
	await expect(dialog).toBeHidden();
});

test('revoked agent offers re-enrollment', async ({ page }) => {
	await mockApiResponse(page, '**/v1/monitoring/agents', {
		schema_version: 1,
		agents: [agentRow({ state: 'revoked' })],
		generated_at_ms: Date.now(),
		truncated: false
	});
	await page.reload();

	const card = page.locator('section', { hasText: 'Guest monitoring agent' });
	await expect(card.getByText('Revoked', { exact: true })).toBeVisible();
	await expect(card.getByText(/no longer report/i)).toBeVisible();
	await expect(card.getByRole('button', { name: 'Enroll agent' })).toBeVisible();
});
