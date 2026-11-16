import { describe, expect, it } from 'vitest';
import { groupDigits } from './digits.js';

describe('groupDigits', () => {
	it('groups by three with a narrow no-break space', () => {
		expect(groupDigits('6500')).toBe('6 500');
		expect(groupDigits('1234567')).toBe('1 234 567');
		expect(groupDigits('999')).toBe('999');
	});
});
