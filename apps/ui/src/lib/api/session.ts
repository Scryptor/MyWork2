// The token of this page's session. The storage is shared by the tabs of the browser (localStorage), so
// the token is read from it once, at the start, and kept in memory: a request goes with the token this
// page signed in with, never with one another tab put there later. When the storage stops holding it
// (another tab signed out, or in as someone else), this page's session is over (`setSessionLostHandler`).
const KEY = 'mw.session';

function read(storage: () => Storage): string | null {
	try {
		return storage().getItem(KEY);
	} catch {
		return null;
	}
}

function remove(storage: () => Storage) {
	try {
		storage().removeItem(KEY);
	} catch {
		// storage is unavailable: nothing to remove
	}
}

const STORAGES = [() => sessionStorage, () => localStorage];

/** What the storage of this tab holds now: its own (sessionStorage) first, then the shared one. */
function stored(): string | null {
	return read(() => sessionStorage) ?? read(() => localStorage);
}

let held: string | null = typeof window === 'undefined' ? null : stored();
/** The token is in the storage (false: the storage would not take it, it lives in memory only). */
let kept = held !== null;
let onLost: () => void = () => {};

/** The token of this page's session; null: nobody is signed in here. */
export function getToken(): string | null {
	return held;
}

/** `remember` keeps the token between browser sessions, otherwise it lives until the tab closes. */
export function setToken(token: string, remember: boolean): void {
	// A sign-in takes the device: whatever token was there goes.
	for (const storage of STORAGES) remove(storage);
	held = token;
	try {
		(remember ? localStorage : sessionStorage).setItem(KEY, token);
		kept = true;
	} catch {
		// storage is unavailable: the user will have to sign in again after a reload
		kept = false;
	}
}

/**
 * This page's session ends (signing out, an expired session): its token goes, from the storage too
 * while it is still there. A token another tab put there meanwhile stays: it is that tab's session.
 */
export function clearToken(): void {
	for (const storage of STORAGES) if (held !== null && read(storage) === held) remove(storage);
	held = null;
}

/** The storage no longer holds this page's token: another tab signed out or in as someone else. */
export function sessionLost(): boolean {
	return held !== null && kept && stored() !== held;
}

/**
 * Ends this page's session after another tab ended it: the token is forgotten here (the storage is that
 * tab's now) and the program is told, once.
 */
export function loseSession(): void {
	if (held === null) return;
	held = null;
	onLost();
}

/** What the program does when another tab ended the session (the `(app)` layout sets it). */
export function setSessionLostHandler(fn: () => void): void {
	onLost = fn;
}

// Another tab changed the shared storage: this page notices at once, not only at its next request.
if (typeof window !== 'undefined')
	window.addEventListener('storage', (e) => {
		if ((e.key === KEY || e.key === null) && sessionLost()) loseSession();
	});
