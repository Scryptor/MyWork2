import { afterEach, describe, expect, it, vi } from 'vitest';
import { readStorage, removeStorage, writeStorage } from './storage.js';

afterEach(() => {
	vi.restoreAllMocks();
	localStorage.clear();
	sessionStorage.clear();
});

describe('storage', () => {
	it('reads null and swallows errors when storage throws', () => {
		vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
			throw new Error('denied');
		});
		vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new Error('denied');
		});
		vi.spyOn(Storage.prototype, 'removeItem').mockImplementation(() => {
			throw new Error('denied');
		});
		expect(readStorage('mw.x')).toBeNull();
		expect(() => writeStorage('mw.x', '1')).not.toThrow();
		expect(() => removeStorage('mw.x')).not.toThrow();
	});

	it('round-trips a value in local storage by default', () => {
		writeStorage('mw.l', 'v');
		expect(localStorage.getItem('mw.l')).toBe('v');
		expect(readStorage('mw.l')).toBe('v');
		removeStorage('mw.l');
		expect(readStorage('mw.l')).toBeNull();
	});

	it('uses sessionStorage for the session area', () => {
		writeStorage('mw.s', 'a', 'session');
		expect(sessionStorage.getItem('mw.s')).toBe('a');
		expect(localStorage.getItem('mw.s')).toBeNull();
		expect(readStorage('mw.s', 'session')).toBe('a');
	});
});
