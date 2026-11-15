import { describe, expect, it } from 'vitest';
import { isAllowed } from './check-licenses.mjs';

describe('isAllowed', () => {
	it('accepts permissive licenses and expressions', () => {
		expect(isAllowed('MIT', 'svelte')).toBe(true);
		expect(isAllowed('(MIT OR Apache-2.0)', 'x')).toBe(true);
		expect(isAllowed('MIT AND ISC', 'x')).toBe(true);
	});

	it('rejects copyleft, weak copyleft, empty and unlicensed', () => {
		expect(isAllowed('MIT AND GPL-3.0', 'x')).toBe(false);
		expect(isAllowed('MPL-2.0', 'x')).toBe(false);
		expect(isAllowed('', 'x')).toBe(false);
		expect(isAllowed('UNLICENSED', 'x')).toBe(false);
	});

	it('allows OFL-1.1 only for fontsource packages', () => {
		expect(isAllowed('OFL-1.1', '@fontsource-variable/inter')).toBe(true);
		expect(isAllowed('OFL-1.1', 'some-icons')).toBe(false);
	});
});
