import { afterEach, expect, it } from 'vitest';
import { clearToken, getToken, setToken } from './session.js';

afterEach(() => {
	clearToken();
	localStorage.clear();
	sessionStorage.clear();
});

it('remember keeps the token in localStorage, otherwise sessionStorage', () => {
	setToken('a', true);
	expect(localStorage.getItem('mw.session')).toBe('a');
	expect(sessionStorage.getItem('mw.session')).toBeNull();
	expect(getToken()).toBe('a');
	setToken('b', false);
	expect(localStorage.getItem('mw.session')).toBeNull();
	expect(sessionStorage.getItem('mw.session')).toBe('b');
	expect(getToken()).toBe('b');
});

it('clearToken removes the token everywhere', () => {
	setToken('a', true);
	clearToken();
	expect(getToken()).toBeNull();
	expect(localStorage.getItem('mw.session')).toBeNull();
});
