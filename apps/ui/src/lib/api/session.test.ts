import { afterEach, describe, expect, it, vi } from 'vitest';
import { clearToken, getToken, sessionLost, setSessionLostHandler, setToken } from './session.js';

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

describe('another tab', () => {
	afterEach(() => setSessionLostHandler(() => {}));

	it('a token another tab put into the storage is not this page’s: its requests keep their own', () => {
		setToken('a', true);
		localStorage.setItem('mw.session', 'b');
		expect(getToken()).toBe('a');
		expect(sessionLost()).toBe(true);
	});

	it('signing out there ends the session here at once, once; the storage is left as it is', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', true);
		// Another tab signs out: the storage event of the shared storage.
		localStorage.removeItem('mw.session');
		window.dispatchEvent(new StorageEvent('storage', { key: 'mw.session' }));
		expect(lost).toHaveBeenCalledOnce();
		expect(getToken()).toBeNull();
		// …and signs in as someone else: nothing more happens here, that token is not taken.
		localStorage.setItem('mw.session', 'b');
		window.dispatchEvent(new StorageEvent('storage', { key: 'mw.session' }));
		expect(lost).toHaveBeenCalledOnce();
		expect(getToken()).toBeNull();
		expect(localStorage.getItem('mw.session')).toBe('b');
	});

	it('other keys and the same token change nothing', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', true);
		window.dispatchEvent(new StorageEvent('storage', { key: 'mw.point' }));
		window.dispatchEvent(new StorageEvent('storage', { key: 'mw.session' }));
		expect(lost).not.toHaveBeenCalled();
		expect(getToken()).toBe('a');
	});

	it('a tab of its own (sessionStorage) is not touched by the shared storage', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', false);
		localStorage.setItem('mw.session', 'b');
		window.dispatchEvent(new StorageEvent('storage', { key: 'mw.session' }));
		expect(lost).not.toHaveBeenCalled();
		expect(getToken()).toBe('a');
	});

	it('an ended session takes only its own token from the storage, not one another tab put there', () => {
		setToken('a', true);
		localStorage.setItem('mw.session', 'b');
		clearToken();
		expect(getToken()).toBeNull();
		expect(localStorage.getItem('mw.session')).toBe('b');
	});
});
