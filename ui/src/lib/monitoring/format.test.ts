import { describe, expect, it } from 'vitest';
import {
	formatAge,
	formatBytes,
	formatCounterRate,
	formatRatio,
	metricTitle,
	qualityLabel,
	reasonLabel,
	sourceLabel
} from './format.ts';
import { monitoringRangeWindow } from '#lib/bff/monitoring.ts';

describe('formatBytes', () => {
	it('formats binary bytes with the correct unit', () => {
		expect(formatBytes(0)).toBe('0 B');
		expect(formatBytes(512)).toBe('512 B');
		expect(formatBytes(1024)).toBe('1.0 KiB');
		expect(formatBytes(1536)).toBe('1.5 KiB');
		expect(formatBytes(1024 * 1024 * 1024)).toBe('1.0 GiB');
		expect(formatBytes(3 * 1024 ** 4)).toBe('3.0 TiB');
	});

	it('never invents a value for missing data', () => {
		expect(formatBytes(Number.NaN)).toBe('—');
	});
});

describe('formatRatio', () => {
	it('renders a 0..1 ratio as percent', () => {
		expect(formatRatio(0)).toBe('0.0%');
		expect(formatRatio(0.42)).toBe('42.0%');
		expect(formatRatio(1)).toBe('100.0%');
	});

	it('never invents a value for missing data', () => {
		expect(formatRatio(Number.NaN)).toBe('—');
	});
});

describe('formatCounterRate', () => {
	it('divides the exact integer delta by the point window', () => {
		expect(formatCounterRate('2000', 15_000)).toBeCloseTo(133.33, 1);
	});

	it('accepts beyond-2^53 decimal strings (display-rate approximation)', () => {
		// The wire keeps the exact integer; the chart rate is a float.
		// Number('9007199254740993') rounds to ...992 — documented,
		// bounded to display precision.
		expect(formatCounterRate('9007199254740993', 1000)).toBe(9007199254740992);
	});

	it('returns null when the delta or window is missing', () => {
		expect(formatCounterRate(undefined, 15_000)).toBeNull();
		expect(formatCounterRate('2000', 0)).toBeNull();
	});
});

describe('formatAge', () => {
	it('renders compact ages', () => {
		const now = 1_700_000_000_000;
		expect(formatAge(now - 5_000, now)).toBe('5s ago');
		expect(formatAge(now - 65_000, now)).toBe('1m ago');
		expect(formatAge(now - 2 * 60 * 60 * 1000, now)).toBe('2h ago');
	});

	it('renders never for absent timestamps', () => {
		expect(formatAge(undefined, 123)).toBe('never');
		expect(formatAge(0, 123)).toBe('never');
	});
});

describe('labels', () => {
	it('maps wire vocabularies to friendly labels', () => {
		expect(sourceLabel('node_os')).toBe('Node OS');
		expect(sourceLabel('vmm')).toBe('VMM');
		expect(sourceLabel(null)).toBe('no source');
		expect(qualityLabel('insufficient_samples')).toBe('insufficient samples');
		expect(reasonLabel('not_collected')).toBe('No data collected yet');
		expect(reasonLabel(undefined)).toBe('No data');
	});

	it('derives chart titles from metric ids', () => {
		expect(metricTitle('vm.cpu.cores_used')).toBe('CPU Cores Used');
		expect(metricTitle('node.memory.available_bytes')).toBe('Memory Available Bytes');
	});
});

describe('monitoringRangeWindow', () => {
	it('maps every view selection to a from/to window ending now', () => {
		const before = Date.now();
		const oneHour = monitoringRangeWindow('1h');
		expect(oneHour.to_ms).toBeGreaterThanOrEqual(before);
		expect(oneHour.to_ms - oneHour.from_ms).toBe(60 * 60 * 1000);

		const thirtyDays = monitoringRangeWindow('30d');
		expect(thirtyDays.to_ms - thirtyDays.from_ms).toBe(30 * 24 * 60 * 60 * 1000);
	});
});
