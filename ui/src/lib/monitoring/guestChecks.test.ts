import { describe, expect, it } from 'vitest';
import {
	checkAgeLabel,
	checkStatusFromCode,
	checkStatusView,
	guestFsRows,
	guestProcessRows,
	parseCheckId,
	parseInterfaceId,
	parseMountId,
	sortChecks
} from './guestChecks.ts';
import type {
	MonitoringCheck,
	MonitoringCurrentSample
} from '#lib/bff/monitoring.ts';

describe('checkStatusView', () => {
	it('covers the full wire vocabulary', () => {
		expect(checkStatusView('ok')).toMatchObject({ label: 'OK', tone: 'success' });
		expect(checkStatusView('warning')).toMatchObject({
			label: 'Warning',
			tone: 'warning'
		});
		expect(checkStatusView('critical')).toMatchObject({
			label: 'Critical',
			tone: 'danger'
		});
		expect(checkStatusView('unknown')).toMatchObject({
			label: 'Unknown',
			tone: 'muted'
		});
	});

	it('never renders unknown as healthy', () => {
		const view = checkStatusView('unknown');
		expect(view.description).toMatch(/could not run|not known/i);
		expect(view.description).not.toMatch(/pass|healthy|ok/i);
	});

	it('falls back to the unknown view for off-vocabulary statuses', () => {
		expect(checkStatusView('bogus').tone).toBe('muted');
		expect(checkStatusView('bogus').label).toBe('Unknown');
	});
});

describe('checkStatusFromCode', () => {
	it('maps the documented codes', () => {
		expect(checkStatusFromCode(0)).toBe('ok');
		expect(checkStatusFromCode(1)).toBe('warning');
		expect(checkStatusFromCode(2)).toBe('critical');
		expect(checkStatusFromCode(3)).toBe('unknown');
	});

	it('treats anything else as unknown, never healthy', () => {
		expect(checkStatusFromCode(4)).toBe('unknown');
		expect(checkStatusFromCode(-1)).toBe('unknown');
		expect(checkStatusFromCode(99)).toBe('unknown');
	});
});

describe('parseCheckId', () => {
	it('splits on the first colon and labels known kinds', () => {
		expect(parseCheckId('service:nginx.service')).toEqual({
			kind: 'service',
			kindLabel: 'Service',
			name: 'nginx.service'
		});
		expect(parseCheckId('http:public-api')).toEqual({
			kind: 'http',
			kindLabel: 'HTTP check',
			name: 'public-api'
		});
		expect(parseCheckId('tcp:db-port')).toEqual({
			kind: 'tcp',
			kindLabel: 'TCP check',
			name: 'db-port'
		});
		expect(parseCheckId('plugin:backup-freshness')).toEqual({
			kind: 'plugin',
			kindLabel: 'Plugin',
			name: 'backup-freshness'
		});
	});

	it('keeps only the first colon as the separator', () => {
		expect(parseCheckId('plugin:com.example:check')).toEqual({
			kind: 'plugin',
			kindLabel: 'Plugin',
			name: 'com.example:check'
		});
	});

	it('falls back to the raw prefix for unknown kinds', () => {
		expect(parseCheckId('custom:thing')).toEqual({
			kind: 'custom',
			kindLabel: 'custom',
			name: 'thing'
		});
	});

	it('is robust to ids without a colon', () => {
		expect(parseCheckId('bare-check')).toEqual({
			kind: '',
			kindLabel: 'Check',
			name: 'bare-check'
		});
	});
});

describe('parseMountId', () => {
	it('splits fstype from mountpoint', () => {
		expect(parseMountId('ext4:/')).toEqual({ fstype: 'ext4', mountpoint: '/' });
		expect(parseMountId('xfs:/var/log')).toEqual({
			fstype: 'xfs',
			mountpoint: '/var/log'
		});
	});

	it('is robust to ids without a colon', () => {
		expect(parseMountId('/data')).toEqual({ fstype: '', mountpoint: '/data' });
	});
});

describe('parseInterfaceId', () => {
	it('labels every documented class', () => {
		expect(parseInterfaceId('phys:eth0')).toEqual({
			ifClass: 'phys',
			classLabel: 'Physical',
			name: 'eth0'
		});
		expect(parseInterfaceId('virt:enp0s3')).toEqual({
			ifClass: 'virt',
			classLabel: 'Virtual',
			name: 'enp0s3'
		});
		expect(parseInterfaceId('bridge:br0')).toEqual({
			ifClass: 'bridge',
			classLabel: 'Bridge',
			name: 'br0'
		});
		expect(parseInterfaceId('loopback:lo')).toEqual({
			ifClass: 'loopback',
			classLabel: 'Loopback',
			name: 'lo'
		});
		expect(parseInterfaceId('other:teammaster0')).toEqual({
			ifClass: 'other',
			classLabel: 'Other',
			name: 'teammaster0'
		});
	});

	it('falls back to the raw prefix for unknown classes', () => {
		expect(parseInterfaceId('wlan:wlp3s0').classLabel).toBe('wlan');
	});

	it('is robust to ids without a colon', () => {
		expect(parseInterfaceId('eth9')).toEqual({
			ifClass: '',
			classLabel: '',
			name: 'eth9'
		});
	});
});

describe('checkAgeLabel', () => {
	const now = 1_000_000_000_000;

	it('states absence honestly instead of zeroing', () => {
		expect(checkAgeLabel(null, now)).toBe('never observed');
		expect(checkAgeLabel(undefined, now)).toBe('never observed');
		expect(checkAgeLabel(0, now)).toBe('never observed');
		expect(checkAgeLabel(Number.NaN, now)).toBe('never observed');
	});

	it('formats ages compactly at the boundaries', () => {
		expect(checkAgeLabel(now, now)).toBe('0s ago');
		expect(checkAgeLabel(now - 59_000, now)).toBe('59s ago');
		expect(checkAgeLabel(now - 60_000, now)).toBe('1m ago');
		expect(checkAgeLabel(now - 3_599_000, now)).toBe('59m ago');
		expect(checkAgeLabel(now - 3_600_000, now)).toBe('1h ago');
		expect(checkAgeLabel(now - 86_399_000, now)).toBe('23h ago');
		expect(checkAgeLabel(now - 86_400_000, now)).toBe('1d ago');
		expect(checkAgeLabel(now - 5 * 86_400_000, now)).toBe('5d ago');
	});

	it('never renders a negative age', () => {
		// A clock skew (observed slightly in the future) must not
		// produce "-3s ago".
		expect(checkAgeLabel(now + 3_000, now)).toBe('0s ago');
	});
});

function sample(
	metricId: string,
	dimensions: Record<string, string>,
	overrides: Partial<MonitoringCurrentSample> = {}
): MonitoringCurrentSample {
	return {
		metric_id: metricId,
		source: 'guest_agent',
		dimensions,
		kind: 'gauge',
		unit: 'bytes',
		observed_at_ms: 1_000_000_000_000,
		received_at_ms: 1_000_000_000_000,
		quality: 'valid',
		stale: false,
		...overrides
	};
}

describe('guestFsRows', () => {
	it('groups series by mount_id and derives usage from both byte samples', () => {
		const rows = guestFsRows([
			sample('vm.guest.fs.total_bytes', { mount_id: 'ext4:/' }, {
				integer_value: '536870912000',
				unit: 'bytes'
			}),
			sample('vm.guest.fs.available_bytes', { mount_id: 'ext4:/' }, {
				integer_value: '268435456000',
				unit: 'bytes'
			}),
			sample('vm.guest.fs.inodes_utilization_ratio', { mount_id: 'ext4:/' }, {
				value: 0.12,
				unit: 'ratio'
			}),
			sample('vm.guest.fs.read_only', { mount_id: 'ext4:/' }, {
				value: 0,
				unit: 'ratio'
			})
		]);
		expect(rows).toHaveLength(1);
		expect(rows[0].mountpoint).toBe('/');
		expect(rows[0].fstype).toBe('ext4');
		expect(rows[0].totalBytes).toBe(536870912000);
		expect(rows[0].availableBytes).toBe(268435456000);
		expect(rows[0].usedBytes).toBe(268435456000);
		expect(rows[0].usageRatio).toBeCloseTo(0.5, 10);
		expect(rows[0].inodeRatio).toBe(0.12);
		expect(rows[0].readOnly).toBe(false);
		expect(rows[0].stale).toBe(false);
	});

	it('marks read_only=1 as read-only and surfaces per-series staleness', () => {
		const rows = guestFsRows([
			sample('vm.guest.fs.read_only', { mount_id: 'xfs:/var' }, {
				value: 1,
				unit: 'ratio'
			}),
			sample('vm.guest.fs.total_bytes', { mount_id: 'xfs:/var' }, {
				integer_value: '107374182400',
				stale: true
			})
		]);
		expect(rows[0].readOnly).toBe(true);
		expect(rows[0].stale).toBe(true);
	});

	it('renders absence honestly: missing samples stay null, never 0', () => {
		const rows = guestFsRows([
			sample('vm.guest.fs.available_bytes', { mount_id: 'ext4:/data' }, {
				integer_value: '1024'
			})
		]);
		expect(rows[0].totalBytes).toBeNull();
		expect(rows[0].usedBytes).toBeNull();
		expect(rows[0].usageRatio).toBeNull();
		expect(rows[0].inodeRatio).toBeNull();
		expect(rows[0].readOnly).toBeNull();
	});

	it('ignores series without a mount_id dimension', () => {
		expect(guestFsRows([sample('vm.guest.fs.total_bytes', {})])).toHaveLength(0);
	});

	it('sorts rows by mountpoint', () => {
		const rows = guestFsRows([
			sample('vm.guest.fs.total_bytes', { mount_id: 'xfs:/var' }),
			sample('vm.guest.fs.total_bytes', { mount_id: 'ext4:/' })
		]);
		expect(rows.map((r) => r.mountpoint)).toEqual(['/', '/var']);
	});
});

describe('guestProcessRows', () => {
	it('groups series by process_selector', () => {
		const rows = guestProcessRows([
			sample('vm.guest.process.count', { process_selector: 'nginx' }, {
				value: 4,
				unit: 'count'
			}),
			sample('vm.guest.process.cpu_utilization_ratio', { process_selector: 'nginx' }, {
				value: 0.25,
				unit: 'ratio'
			}),
			sample('vm.guest.process.rss_bytes', { process_selector: 'nginx' }, {
				integer_value: '134217728'
			})
		]);
		expect(rows).toHaveLength(1);
		expect(rows[0].selector).toBe('nginx');
		expect(rows[0].count).toBe(4);
		expect(rows[0].cpuRatio).toBe(0.25);
		expect(rows[0].rssBytes).toBe(134217728);
		expect(rows[0].stale).toBe(false);
	});

	it('keeps missing metrics null and flags stale samples', () => {
		const rows = guestProcessRows([
			sample('vm.guest.process.count', { process_selector: 'postgres' }, {
				value: 1,
				unit: 'count',
				stale: true
			})
		]);
		expect(rows[0].cpuRatio).toBeNull();
		expect(rows[0].rssBytes).toBeNull();
		expect(rows[0].stale).toBe(true);
	});
});

describe('sortChecks', () => {
	function check(checkId: string): MonitoringCheck {
		return {
			check_id: checkId,
			service_key: checkId,
			status: 'ok',
			summary: null,
			observed_at_ms: null,
			received_at_ms: null,
			agent_id: null,
			stale: false
		};
	}

	it('orders services first, then http/tcp, then plugins, then check_id', () => {
		const sorted = sortChecks([
			check('plugin:zzz'),
			check('http:bbb'),
			check('service:zzz.service'),
			check('tcp:aaa'),
			check('plugin:aaa'),
			check('service:aaa.service'),
			check('http:aaa')
		]);
		expect(sorted.map((c) => c.check_id)).toEqual([
			'service:aaa.service',
			'service:zzz.service',
			'http:aaa',
			'http:bbb',
			'tcp:aaa',
			'plugin:aaa',
			'plugin:zzz'
		]);
	});

	it('sorts unknown kinds after the documented ones', () => {
		const sorted = sortChecks([check('custom:x'), check('plugin:y')]);
		expect(sorted.map((c) => c.check_id)).toEqual(['plugin:y', 'custom:x']);
	});

	it('does not mutate the input array', () => {
		const input = [check('plugin:a'), check('service:b.service')];
		sortChecks(input);
		expect(input[0].check_id).toBe('plugin:a');
	});
});
