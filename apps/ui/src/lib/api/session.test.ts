import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	adoptStored,
	clearToken,
	followStorage,
	getToken,
	noteUser,
	setSessionFoundHandler,
	setSessionLostHandler,
	setToken
} from './session.js';

const KEY = 'mw.session';
const value = (token: string, userId: number | null) => JSON.stringify({ token, user_id: userId });
/** Another tab writes the shared storage; this page hears of it by the storage event. */
function otherTab(raw: string | null) {
	if (raw === null) localStorage.removeItem(KEY);
	else localStorage.setItem(KEY, raw);
	window.dispatchEvent(new StorageEvent('storage', { key: KEY }));
}

afterEach(() => {
	clearToken();
	localStorage.clear();
	sessionStorage.clear();
	setSessionLostHandler(() => {});
	setSessionFoundHandler(() => {});
	vi.restoreAllMocks();
});

it('remember keeps the session in localStorage, otherwise sessionStorage, with its user', () => {
	setToken('a', true, 7);
	expect(localStorage.getItem(KEY)).toBe(value('a', 7));
	expect(sessionStorage.getItem(KEY)).toBeNull();
	expect(getToken()).toBe('a');
	setToken('b', false, 8);
	// Another user's remembered session goes from the device.
	expect(localStorage.getItem(KEY)).toBeNull();
	expect(sessionStorage.getItem(KEY)).toBe(value('b', 8));
	expect(getToken()).toBe('b');
});

it('clearToken removes the token everywhere', () => {
	setToken('a', true, 7);
	clearToken();
	expect(getToken()).toBeNull();
	expect(localStorage.getItem(KEY)).toBeNull();
});

it('a new sign-in is written in one step: the shared session is not removed first', () => {
	setToken('a', true, 7);
	const removed = vi.spyOn(Storage.prototype, 'removeItem');
	setToken('b', true, 7);
	expect(removed.mock.contexts).not.toContain(localStorage);
	expect(localStorage.getItem(KEY)).toBe(value('b', 7));
	// Without «remember» the same user's remembered session stays for the other tabs.
	setToken('c', false, 7);
	expect(removed.mock.contexts).not.toContain(localStorage);
	expect(localStorage.getItem(KEY)).toBe(value('b', 7));
});

describe('another tab', () => {
	it('signs in again as the same user: this page takes the new token, nothing else happens', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', true, 7);
		otherTab(value('b', 7));
		expect(lost).not.toHaveBeenCalled();
		expect(getToken()).toBe('b');
	});

	it('signs in as another user: this page’s session ends, once; the storage is left as it is', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', true, 7);
		otherTab(value('b', 8));
		expect(lost).toHaveBeenCalledOnce();
		expect(getToken()).toBeNull();
		expect(localStorage.getItem(KEY)).toBe(value('b', 8));
	});

	it('signs out: this page’s session ends, once', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', true, 7);
		otherTab(null);
		expect(lost).toHaveBeenCalledOnce();
		expect(getToken()).toBeNull();
	});

	it('a session whose user is not known yet is not taken up; once /auth/me says it, it is', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		// A bare token, stored by an earlier version.
		localStorage.setItem(KEY, 'a');
		expect(adoptStored()?.token).toBe('a');
		noteUser(7);
		otherTab(value('b', 7));
		expect(lost).not.toHaveBeenCalled();
		expect(getToken()).toBe('b');
		clearToken();

		localStorage.setItem(KEY, 'c');
		adoptStored();
		otherTab(value('d', 7));
		expect(lost).toHaveBeenCalledOnce();
	});

	it('other keys and the same session change nothing', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', true, 7);
		window.dispatchEvent(new StorageEvent('storage', { key: 'mw.point' }));
		window.dispatchEvent(new StorageEvent('storage', { key: KEY }));
		expect(lost).not.toHaveBeenCalled();
		expect(getToken()).toBe('a');
	});

	it('a tab of its own (sessionStorage) is not touched by the shared storage', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		setToken('a', false, 7);
		otherTab(value('b', 8));
		expect(lost).not.toHaveBeenCalled();
		expect(getToken()).toBe('a');
	});

	it('a page without a session hears of a session stored there, and takes it up', () => {
		const found = vi.fn(() => adoptStored());
		const lost = vi.fn();
		setSessionFoundHandler(found);
		setSessionLostHandler(lost);
		otherTab(value('b', 7));
		expect(found).toHaveBeenCalledOnce();
		expect(getToken()).toBe('b');
		expect(lost).not.toHaveBeenCalled();
		// A page with a session follows the storage instead.
		otherTab(value('c', 7));
		expect(found).toHaveBeenCalledOnce();
		expect(getToken()).toBe('c');
	});

	it('followStorage before a request does the same without the event', () => {
		setToken('a', true, 7);
		localStorage.setItem(KEY, value('b', 7));
		expect(followStorage()).toBe(false);
		expect(getToken()).toBe('b');
		localStorage.setItem(KEY, value('c', 8));
		expect(followStorage()).toBe(true);
	});

	it('an ended session takes only its own token from the storage, not one another tab put there', () => {
		setToken('a', true, 7);
		localStorage.setItem(KEY, value('b', 8));
		clearToken();
		expect(getToken()).toBeNull();
		expect(localStorage.getItem(KEY)).toBe(value('b', 8));
	});

	it('a storage that does not take the session keeps it in memory, not lost', () => {
		const lost = vi.fn();
		setSessionLostHandler(lost);
		const set = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new DOMException('full', 'QuotaExceededError');
		});
		try {
			setToken('a', true, 7);
		} finally {
			set.mockRestore();
		}
		expect(getToken()).toBe('a');
		expect(followStorage()).toBe(false);
		window.dispatchEvent(new StorageEvent('storage', { key: KEY }));
		expect(lost).not.toHaveBeenCalled();
	});
});
