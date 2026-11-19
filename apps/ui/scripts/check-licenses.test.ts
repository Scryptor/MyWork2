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

	it('binds AND tighter than OR and keeps parentheses', () => {
		expect(isAllowed('GPL-3.0 AND (MIT OR Apache-2.0)', 'x')).toBe(false);
		expect(isAllowed('(MIT OR GPL-3.0) AND Apache-2.0', 'x')).toBe(true);
		expect(isAllowed('MIT OR GPL-3.0 AND Apache-2.0', 'x')).toBe(true);
		expect(isAllowed('MIT AND GPL-3.0 OR MPL-2.0', 'x')).toBe(false);
	});

	it('treats WITH as part of the license and refuses malformed expressions', () => {
		expect(isAllowed('GPL-2.0 WITH Classpath-exception-2.0', 'x')).toBe(false);
		expect(isAllowed('(MIT OR Apache-2.0', 'x')).toBe(false);
		expect(isAllowed('MIT OR', 'x')).toBe(false);
		expect(isAllowed('MIT Apache-2.0', 'x')).toBe(false);
	});

	it('allows OFL-1.1 only for fontsource packages', () => {
		expect(isAllowed('OFL-1.1', '@fontsource-variable/inter')).toBe(true);
		expect(isAllowed('OFL-1.1', 'some-icons')).toBe(false);
	});
});
