import { describe, expect, it } from 'vitest';

import { getTaskFailureCause } from './task-failure';

describe('task-failure cause helper (#502)', () => {
	it('returns the recorded code and refusal text verbatim', () => {
		const cause = getTaskFailureCause({
			error_code: 'UNSUPPORTED_BY_AGENT',
			error_message: 'snapshot_volume is unsupported in core-managed mode'
		});
		// Verbatim — the diagnostic is never rewritten, trimmed of
		// meaning, or defaulted to a generic string.
		expect(cause).toEqual({
			code: 'UNSUPPORTED_BY_AGENT',
			message: 'snapshot_volume is unsupported in core-managed mode'
		});
	});

	it('returns null for a succeeded/running op — nothing is recorded, nothing is fabricated', () => {
		expect(getTaskFailureCause({ error_code: null, error_message: null })).toBeNull();
		expect(getTaskFailureCause({})).toBeNull();
		expect(getTaskFailureCause(undefined)).toBeNull();
	});

	it('returns null for a terminal op with no recorded cause — never a placeholder like UNKNOWN', () => {
		// A pre-#498 writer (or any arm that journals a terminal Failed
		// row without a diagnostic) must render exactly as before:
		// absent fields, not a fabricated cause.
		expect(getTaskFailureCause({ error_code: null, error_message: null })).toBeNull();
	});

	it('treats blank strings as absent', () => {
		expect(getTaskFailureCause({ error_code: '', error_message: '' })).toBeNull();
		expect(getTaskFailureCause({ error_code: '   ', error_message: null })).toBeNull();
	});

	it('surfaces a message without a code (code renders null, not a placeholder)', () => {
		const cause = getTaskFailureCause({
			error_code: null,
			error_message: 'dispatch failed: node unreachable'
		});
		expect(cause).toEqual({ code: null, message: 'dispatch failed: node unreachable' });
	});

	it('surfaces a code without a message', () => {
		const cause = getTaskFailureCause({ error_code: 'DISPATCH_FAILED', error_message: null });
		expect(cause).toEqual({ code: 'DISPATCH_FAILED', message: null });
	});

	it('keeps the other recorded codes on their recorded shape', () => {
		// The #498/#500 fast-fail vocabulary — pinned so a future
		// writer-side rename is visible in this tier.
		for (const code of ['UNSUPPORTED_BY_AGENT', 'DISPATCH_FAILED', 'AGENT_REJECTED', 'MIGRATION_FAILED']) {
			expect(getTaskFailureCause({ error_code: code, error_message: 'refused' })?.code).toBe(code);
		}
	});
});
