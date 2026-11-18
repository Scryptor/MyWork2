import { beforeEach, describe, expect, it } from 'vitest';
import { prefs } from '#lib/design/prefs.svelte.js';
import { scaleLabel } from './size.js';

describe('scaleLabel', () => {
	it('writes percents', () => {
		expect(scaleLabel(1.1)).toBe('110 %');
		expect(scaleLabel(0.9)).toBe('90 %');
		expect(scaleLabel(1.25)).toBe('125 %');
	});
});

describe('size keys', () => {
	beforeEach(() => prefs.set({ scale: 1.5 }));
	it('do not step past the ends', async () => {
		const { stepSize } = await import('./size.js');
		stepSize(1);
		expect(prefs.value.scale).toBe(1.5);
	});
});
