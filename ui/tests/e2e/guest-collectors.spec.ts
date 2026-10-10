import { test, expect, type Page } from '@playwright/test';
import { loginAsAdmin, mockApiResponse } from './helpers';

/**
 * Guest-side telemetry surfaces (campaign #602 G4, prompt 04).
 *
 * Wire shapes under test:
 *   - POST /v1/monitoring/current -> dimensioned guest collector series
 *     (fs bytes/inodes/read_only by `mount_id`, process count/cpu/rss
 *     by `process_selector`; integer values as decimal strings)
 *   - POST /v1/monitoring/checks  -> guest check results (status
 *     vocabulary, staleness, namespaced check ids)
 *
 * The cards must render every sample honestly (missing series are
 * "—", never 0), badge stale records, and never leak one VM's guest
 * telemetry onto another VM's page.
 */

const NOW = Date.now();

function vmDetail(vmId: string, name: string) {
	return {
		summary: {
			vm_id: vmId,
			name,
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
}

interface SampleOverrides {
	value?: number;
	integer_value?: string;
	unit?: string;
	kind?: string;
	stale?: boolean;
	observedAtOffsetMs?: number;
}

function sample(
	metricId: string,
	dimensions: Record<string, string>,
	overrides: SampleOverrides = {}
) {
	const {
		observedAtOffsetMs = 5_000,
		value,
		integer_value,
		unit = 'bytes',
		kind = 'gauge',
		stale = false
	} = overrides;
	return {
		metric_id: metricId,
		source: 'guest_agent',
		dimensions,
		kind,
		unit,
		observed_at_ms: NOW - observedAtOffsetMs,
		received_at_ms: NOW - observedAtOffsetMs + 1_000,
		quality: 'valid',
		stale,
		...(value !== undefined ? { value } : {}),
		...(integer_value !== undefined ? { integer_value } : {})
	};
}

// Byte-compatible with MonitoringCurrentSample: byte metrics carry
// integer_value as a decimal string, ratios/counters carry value.
const GUEST_CURRENT_VM_1 = {
	schema_version: 1,
	target_kind: 'vm',
	target_id: 'vm-1',
	generated_at_ms: NOW,
	samples: [
		sample('vm.guest.fs.total_bytes', { mount_id: 'ext4:/' }, { integer_value: '536870912000' }),
		sample('vm.guest.fs.available_bytes', { mount_id: 'ext4:/' }, { integer_value: '268435456000' }),
		sample('vm.guest.fs.inodes_utilization_ratio', { mount_id: 'ext4:/' }, { unit: 'ratio', value: 0.041 }),
		sample('vm.guest.fs.read_only', { mount_id: 'ext4:/' }, { kind: 'state', unit: 'boolean', integer_value: '0' }),
		sample(
			'vm.guest.fs.total_bytes',
			{ mount_id: 'xfs:/var' },
			{ integer_value: '10737418240', stale: true, observedAtOffsetMs: 400_000 }
		),
		sample(
			'vm.guest.fs.available_bytes',
			{ mount_id: 'xfs:/var' },
			{ integer_value: '536870912', stale: true, observedAtOffsetMs: 400_000 }
		),
		sample(
			'vm.guest.fs.inodes_utilization_ratio',
			{ mount_id: 'xfs:/var' },
			{ unit: 'ratio', value: 0.87, stale: true, observedAtOffsetMs: 400_000 }
		),
		sample(
			'vm.guest.fs.read_only',
			{ mount_id: 'xfs:/var' },
			{ kind: 'state', unit: 'boolean', integer_value: '1', stale: true, observedAtOffsetMs: 400_000 }
		),
		sample('vm.guest.process.count', { process_selector: 'nginx' }, { unit: 'count', value: 4 }),
		sample('vm.guest.process.cpu_utilization_ratio', { process_selector: 'nginx' }, { unit: 'ratio', value: 0.12 }),
		sample('vm.guest.process.rss_bytes', { process_selector: 'nginx' }, { integer_value: '134217728' }),
		sample(
			'vm.guest.process.count',
			{ process_selector: 'postgres' },
			{ unit: 'count', value: 1, stale: true, observedAtOffsetMs: 300_000 }
		),
		sample(
			'vm.guest.process.cpu_utilization_ratio',
			{ process_selector: 'postgres' },
			{ unit: 'ratio', value: 0.03, stale: true, observedAtOffsetMs: 300_000 }
		),
		sample(
			'vm.guest.process.rss_bytes',
			{ process_selector: 'postgres' },
			{ integer_value: '805306368', stale: true, observedAtOffsetMs: 300_000 }
		)
	]
};

// All four statuses, service/http/plugin kinds, one stale record.
const GUEST_CHECKS_VM_1 = {
	schema_version: 1,
	target_kind: 'vm',
	target_id: 'vm-1',
	generated_at_ms: NOW,
	checks: [
		{
			check_id: 'service:nginx.service',
			service_key: 'nginx.service',
			status: 'ok',
			summary: 'active (running)',
			observed_at_ms: NOW - 9_000,
			received_at_ms: NOW - 8_000,
			agent_id: 'agent-vm-1',
			stale: false
		},
		{
			check_id: 'service:postgresql.service',
			service_key: 'postgresql.service',
			status: 'critical',
			summary: 'failed (Result: exit-code)',
			observed_at_ms: NOW - 12_000,
			received_at_ms: NOW - 11_000,
			agent_id: 'agent-vm-1',
			stale: false
		},
		{
			check_id: 'http:public-api',
			service_key: 'public-api',
			status: 'warning',
			summary: '500 Internal Server Error in 230ms',
			observed_at_ms: NOW - 15_000,
			received_at_ms: NOW - 14_000,
			agent_id: 'agent-vm-1',
			stale: false
		},
		{
			check_id: 'plugin:backup-freshness',
			service_key: 'backup-freshness',
			status: 'unknown',
			summary: 'plugin did not report a result',
			observed_at_ms: NOW - 400_000,
			received_at_ms: NOW - 399_000,
			agent_id: 'agent-vm-1',
			stale: true
		}
	]
};

const EMPTY_CURRENT = {
	schema_version: 1,
	target_kind: 'vm',
	target_id: 'vm-2',
	generated_at_ms: NOW,
	samples: []
};

const EMPTY_CHECKS = {
	schema_version: 1,
	target_kind: 'vm',
	target_id: 'vm-2',
	generated_at_ms: NOW,
	checks: []
};

/** The SectionCard that owns the given card title heading. */
function guestCard(page: Page, title: string) {
	return page
		.getByRole('heading', { name: title })
		.locator('xpath=ancestor::section[1]');
}

test.beforeEach(async ({ page }) => {
	await loginAsAdmin(page);
});

test('renders dimensioned filesystem, process and check telemetry', async ({ page }) => {
	await mockApiResponse(page, '**/v1/vms/get', vmDetail('vm-1', 'web-server'));
	await mockApiResponse(page, '**/v1/monitoring/current', GUEST_CURRENT_VM_1);
	await mockApiResponse(page, '**/v1/monitoring/checks', GUEST_CHECKS_VM_1);
	await page.goto('/vms/vm-1?tab=metrics');
	await expect(page.getByRole('heading', { name: /web-server/i })).toBeVisible();

	const fsCard = guestCard(page, 'Guest filesystems');
	const rootRow = fsCard.getByRole('row', { name: /ext4/ });
	await expect(rootRow).toBeVisible();
	await expect(rootRow).toContainText('/');
	await expect(rootRow).toContainText('500 GiB');
	await expect(rootRow).toContainText('250 GiB');
	await expect(rootRow).toContainText('50.0%');
	await expect(rootRow).toContainText('4.1%');
	const varRow = fsCard.getByRole('row', { name: /xfs/ });
	await expect(varRow).toContainText('/var');
	await expect(varRow).toContainText('9.5 GiB');
	await expect(varRow).toContainText('95.0%');
	await expect(varRow).toContainText('87.0%');
	await expect(varRow).toContainText('read-only');
	await expect(varRow).toContainText('stale');

	const checksCard = guestCard(page, 'Guest checks');
	await expect(checksCard.getByText('OK', { exact: true })).toBeVisible();
	await expect(checksCard.getByText('Warning', { exact: true })).toBeVisible();
	await expect(checksCard.getByText('Critical', { exact: true })).toBeVisible();
	await expect(checksCard.getByText('Unknown', { exact: true })).toBeVisible();
	// Services first, then http, then plugins, then by check_id.
	const rows = checksCard.getByRole('row');
	await expect(rows.nth(1)).toContainText('nginx.service');
	await expect(rows.nth(1)).toContainText('active (running)');
	await expect(rows.nth(2)).toContainText('postgresql.service');
	await expect(rows.nth(3)).toContainText('public-api');
	await expect(rows.nth(4)).toContainText('backup-freshness');
	// Summaries render as plain text; the stale record carries its badge.
	await expect(checksCard.getByText('plugin did not report a result')).toBeVisible();
	await expect(
		checksCard.getByRole('row', { name: /backup-freshness/ }).getByText('stale')
	).toBeVisible();

	const procCard = guestCard(page, 'Guest processes');
	const nginxRow = procCard.getByRole('row', { name: /nginx/ });
	await expect(nginxRow).toContainText('4');
	await expect(nginxRow).toContainText('12.0%');
	await expect(nginxRow).toContainText('128 MiB');
	const pgRow = procCard.getByRole('row', { name: /postgres/ });
	await expect(pgRow).toContainText('768 MiB');
	await expect(pgRow).toContainText('stale');

	// Data is present: the honest empty states must NOT be shown.
	await expect(fsCard.getByText(/No filesystem telemetry/i)).toHaveCount(0);
	await expect(checksCard.getByText(/No checks configured/i)).toHaveCount(0);
	await expect(procCard.getByText(/No process telemetry/i)).toHaveCount(0);
});

test('does not leak one VM guest telemetry onto another VM page', async ({ page }) => {
	await mockApiResponse(page, '**/v1/vms/get', vmDetail('vm-2', 'db-server'));
	// A per-target BFF: vm-1's rich fixture, vm-2's honest empty store.
	// The UI must ask for vm-2 (never vm-1) and render only the answer.
	const requestedTargetIds: string[] = [];
	await page.route('**/v1/monitoring/current', async (route) => {
		const body = (route.request().postDataJSON() ?? {}) as { target_id?: string };
		requestedTargetIds.push(body.target_id ?? '');
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(body.target_id === 'vm-1' ? GUEST_CURRENT_VM_1 : EMPTY_CURRENT)
		});
	});
	await page.route('**/v1/monitoring/checks', async (route) => {
		const body = (route.request().postDataJSON() ?? {}) as { target_id?: string };
		requestedTargetIds.push(body.target_id ?? '');
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(body.target_id === 'vm-1' ? GUEST_CHECKS_VM_1 : EMPTY_CHECKS)
		});
	});
	await page.goto('/vms/vm-2?tab=metrics');
	await expect(page.getByRole('heading', { name: /db-server/i })).toBeVisible();

	// Every guest-telemetry request targeted vm-2.
	expect(requestedTargetIds.length).toBeGreaterThan(0);
	for (const targetId of requestedTargetIds) {
		expect(targetId).toBe('vm-2');
	}

	// None of vm-1's process selector or check names may appear.
	for (const vm1Only of [
		'nginx.service',
		'postgresql.service',
		'public-api',
		'backup-freshness',
		'postgres'
	]) {
		await expect(page.getByText(vm1Only)).toHaveCount(0);
	}

	// vm-2 gets its own honest empty states instead.
	await expect(guestCard(page, 'Guest filesystems').getByText(/No filesystem telemetry/i)).toBeVisible();
	await expect(guestCard(page, 'Guest checks').getByText(/No checks configured/i)).toBeVisible();
	await expect(guestCard(page, 'Guest processes').getByText(/No process telemetry/i)).toBeVisible();
});
