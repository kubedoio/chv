import { describe, expect, it } from 'vitest';
import {
	RULE_TEMPLATES,
	SILENCE_PICKS,
	formatAlertTimestamp,
	incidentStateView,
	ruleSpecSummary,
	severityBadgeVariant,
	silenceRemainingLabel,
	targetHref,
	type RuleTemplate
} from './rules.ts';
import type { AlertRule } from '#lib/bff/alerting.ts';

function rule(partial: Partial<AlertRule>): AlertRule {
	return {
		rule_id: 'r1',
		name: 'test rule',
		enabled: false,
		target_kind: 'vm',
		target_id: 'vm-1',
		rule_type: 'threshold',
		severity: 'warning',
		for_seconds: 300,
		recovery_seconds: 120,
		missing_data: 'unknown',
		revision: 1,
		created_by: 'admin',
		created_at_ms: 0,
		updated_at_ms: 0,
		...partial
	};
}

describe('incidentStateView', () => {
	it('covers the full wire vocabulary', () => {
		expect(incidentStateView('pending')).toMatchObject({ label: 'Pending', tone: 'warning' });
		expect(incidentStateView('firing')).toMatchObject({ label: 'Firing', tone: 'danger' });
		expect(incidentStateView('resolved')).toMatchObject({ label: 'Resolved', tone: 'success' });
	});

	it('never renders an off-vocabulary status as resolved', () => {
		const view = incidentStateView('bogus');
		expect(view.tone).not.toBe('success');
		expect(view.label).toBe('bogus');
	});
});

describe('severityBadgeVariant', () => {
	it('maps the three severities', () => {
		expect(severityBadgeVariant('critical')).toBe('danger');
		expect(severityBadgeVariant('warning')).toBe('warning');
		expect(severityBadgeVariant('info')).toBe('info');
		expect(severityBadgeVariant('other')).toBe('default');
	});
});

describe('targetHref', () => {
	it('links nodes and vms to their detail pages', () => {
		expect(targetHref('node', 'n1')).toBe('/nodes/n1');
		expect(targetHref('vm', 'v1')).toBe('/vms/v1');
		expect(targetHref('other', 'x')).toBe('#');
	});
});

describe('ruleSpecSummary', () => {
	it('renders a threshold with operator symbol', () => {
		const summary = ruleSpecSummary(
			rule({
				rule_type: 'threshold',
				metric_id: 'vm.cpu.capacity_ratio',
				operator: 'greater_than',
				threshold: 0.9
			})
		);
		expect(summary).toBe('vm.cpu.capacity_ratio > 0.9');
	});

	it('renders a threshold dimension match and byte magnitudes', () => {
		const summary = ruleSpecSummary(
			rule({
				rule_type: 'threshold',
				metric_id: 'vm.guest.fs.available_bytes',
				dimension_match: { mount_id: 'ext4:/' },
				operator: 'less_than',
				threshold: 2_147_483_648
			})
		);
		expect(summary).toBe('vm.guest.fs.available_bytes < 2 GiB (mount_id=ext4:/)');
	});

	it('renders a rate with per-second threshold and window', () => {
		const summary = ruleSpecSummary(
			rule({
				rule_type: 'rate',
				metric_id: 'vm.guest.net.rx_errors_total',
				operator: 'greater_than',
				threshold_per_second: 10,
				window_seconds: 300
			})
		);
		expect(summary).toBe('vm.guest.net.rx_errors_total > 10/s over 300s');
	});

	it('renders availability as a staleness condition', () => {
		const summary = ruleSpecSummary(
			rule({
				rule_type: 'availability',
				metric_id: 'node.cpu.capacity_ratio'
			})
		);
		expect(summary).toBe('no data: node.cpu.capacity_ratio');
	});

	it('renders check status', () => {
		const summary = ruleSpecSummary(
			rule({
				rule_type: 'check_status',
				check_id: 'service:nginx.service',
				status_match: 'critical'
			})
		);
		expect(summary).toBe('service:nginx.service is critical');
	});

	it('renders a group with its join operator', () => {
		const summary = ruleSpecSummary(
			rule({
				rule_type: 'group',
				op: 'and',
				conditions: [
					rule({
						rule_id: 'c1',
						rule_type: 'threshold',
						metric_id: 'node.cpu.load1',
						operator: 'greater_than',
						threshold: 8
					}),
					rule({
						rule_id: 'c2',
						rule_type: 'check_status',
						check_id: 'service:nginx.service',
						status_match: 'critical'
					})
				]
			})
		);
		expect(summary).toBe(
			'node.cpu.load1 > 8 AND service:nginx.service is critical'
		);
	});
});

describe('silence helpers', () => {
	it('offers exactly the 30m / 2h / 24h quick picks', () => {
		expect(SILENCE_PICKS).toEqual([
			{ label: '30m', minutes: 30 },
			{ label: '2h', minutes: 120 },
			{ label: '24h', minutes: 1440 }
		]);
	});

	it('labels remaining silence and treats expiry honestly', () => {
		const now = 1_000_000_000;
		expect(silenceRemainingLabel(now + 90_000, now)).toBe('1m left');
		expect(silenceRemainingLabel(now + 7_200_000, now)).toBe('2h left');
		expect(silenceRemainingLabel(now - 1, now)).toBe('expired');
		expect(silenceRemainingLabel(null, now)).toBe('');
	});
});

describe('formatAlertTimestamp', () => {
	it('renders epoch millis and ISO strings, absence as a dash', () => {
		expect(formatAlertTimestamp(0)).not.toBe('—');
		expect(formatAlertTimestamp('2026-10-10T12:00:00Z')).not.toBe('—');
		expect(formatAlertTimestamp(null)).toBe('—');
		expect(formatAlertTimestamp(undefined)).toBe('—');
		expect(formatAlertTimestamp('not a date')).toBe('—');
	});
});

describe('RULE_TEMPLATES', () => {
	// The v1 metric registry allowlist
	// (crates/chv-monitoring-core/src/registry.rs, static REGISTRY).
	// Every template metric_id must be listed there or the BFF rejects
	// the created rule with 400 UNKNOWN_METRIC.
	const REGISTRY_METRIC_IDS = new Set([
		'node.cpu.capacity_ratio',
		'node.cpu.load1',
		'node.cpu.load5',
		'node.cpu.load15',
		'node.memory.total_bytes',
		'node.memory.available_bytes',
		'node.swap.used_bytes',
		'node.memory.psi_some_ratio',
		'node.fs.available_bytes',
		'node.fs.total_bytes',
		'node.net.rx_bytes_total',
		'node.net.tx_bytes_total',
		'node.block.read_bytes_total',
		'node.block.write_bytes_total',
		'vm.cpu.cores_used',
		'vm.cpu.capacity_ratio',
		'vm.cpu.assigned_vcpus',
		'vm.memory.provisioned_bytes',
		'vm.memory.host_accounted_bytes',
		'vm.memory.guest_available_bytes',
		'vm.block.read_bytes_total',
		'vm.block.write_bytes_total',
		'vm.net.rx_bytes_total',
		'vm.net.tx_bytes_total',
		'vm.guest.fs.available_bytes',
		'vm.guest.fs.total_bytes',
		'vm.guest.fs.inodes_utilization_ratio',
		'vm.guest.fs.read_only',
		'vm.guest.service.up',
		'vm.guest.process.count',
		'vm.guest.process.cpu_utilization_ratio',
		'vm.guest.process.rss_bytes',
		'vm.guest.cpu.utilization_ratio',
		'vm.guest.load1',
		'vm.guest.uptime_seconds',
		'vm.guest.net.rx_errors_total',
		'vm.guest.net.tx_errors_total',
		'vm.guest.net.rx_drops_total',
		'vm.guest.net.tx_drops_total',
		'vm.guest.net.rx_bytes_total',
		'vm.guest.net.tx_bytes_total',
		'vm.guest.net.link_up',
		'vm.guest.net.tcp_established',
		'check.duration_seconds',
		'check.status',
		'monitoring.agent.last_seen_age_seconds'
	]);

	it('has exactly the seven starter templates with unique ids', () => {
		expect(RULE_TEMPLATES.map((t) => t.id)).toEqual([
			'node-unreachable',
			'collector-stale',
			'vm-cpu-pressure',
			'vm-storage-near-full',
			'agent-disconnected',
			'guest-fs-inodes-full',
			'guest-service-down'
		]);
	});

	it('uses only real registry metric ids', () => {
		for (const template of RULE_TEMPLATES) {
			if (template.spec.metric_id) {
				expect(
					REGISTRY_METRIC_IDS.has(template.spec.metric_id),
					`template ${template.id} references unknown metric ${template.spec.metric_id}`
				).toBe(true);
			}
		}
	});

	it('uses check ids from the documented namespaced vocabulary', () => {
		for (const template of RULE_TEMPLATES as RuleTemplate[]) {
			if (template.spec.check_id) {
				expect(template.spec.check_id).toMatch(/^(service|http|tcp|plugin):[^\s]+$/);
			}
		}
	});

	it('carries no enabled flag — templates never auto-enable', () => {
		for (const template of RULE_TEMPLATES) {
			expect('enabled' in template).toBe(false);
		}
	});

	it('keeps every template within the spec bounds the BFF enforces', () => {
		for (const template of RULE_TEMPLATES) {
			expect(template.for_seconds).toBeGreaterThan(0);
			expect(template.recovery_seconds).toBeGreaterThan(0);
			if (template.rule_type === 'rate') {
				expect(template.spec.window_seconds ?? 0).toBeGreaterThanOrEqual(30);
				expect(template.spec.window_seconds ?? 0).toBeLessThanOrEqual(3600);
			}
		}
	});
});
