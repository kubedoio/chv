import { describe, expect, it } from 'vitest';
import { load } from '../../routes/overview/+page';

describe('legacy overview route', () => {
	it('redirects /overview to /', async () => {
		expect.assertions(2);

		try {
			await load({} as never);
		} catch (error) {
			const redirectError = error as { status?: number; location?: string };
			expect(redirectError.status).toBe(307);
			expect(redirectError.location).toBe('/');
		}
	});
});
