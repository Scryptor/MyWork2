// The session of this page: its token and whose it is. The storage is shared by the tabs of the browser
// (localStorage), so the session is read from it once, at the start, and kept in memory: a request goes
// with the token of this page's user, never with one of another user that another tab put there later.
// When another tab changes the stored session:
// - a new token of the same user (signed in again): this page takes it up, nothing else changes;
// - another user, or no session at all (signed out): this page's session is over (`setSessionLostHandler`);
//   (an expiry is told from it: the tab where the session expired leaves a mark `{ expired, user_id }`);
// - a tab of its own (signed in without «remember», sessionStorage) is reset too when another user signs
//   in to the shared storage: a shared computer must not keep the earlier employee open;
// - a page without a session (the login page) takes a session that appears (`setSessionFoundHandler`).
const KEY = 'mw.session';

/** Why a page's session is over: it expired (in another tab), or another tab signed out / in as someone else. */
export type LostReason = 'expired' | 'changed';

/** What the storage keeps: the token and the user it belongs to (null: not known yet). */
export interface StoredSession {
	token: string;
	userId: number | null;
}

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

/** `{"token": …, "user_id": …}`; a bare token was stored by an earlier version (its user not known). */
function parse(raw: string | null): StoredSession | null {
	if (raw === null || raw === '') return null;
	try {
		const v: unknown = JSON.parse(raw);
		if (typeof v === 'object' && v !== null && (v as { expired?: unknown }).expired === true)
			return null;
		if (
			typeof v === 'object' &&
			v !== null &&
			typeof (v as { token?: unknown }).token === 'string'
		) {
			const { token, user_id } = v as { token: string; user_id?: unknown };
			return { token, userId: typeof user_id === 'number' ? user_id : null };
		}
	} catch {
		// not JSON: a bare token
	}
	return { token: raw, userId: null };
}

/** The user of the expiry mark in the shared storage; undefined: there is no mark. */
function expiryMark(): { userId: number | null } | undefined {
	const raw = read(() => localStorage);
	if (raw === null) return undefined;
	try {
		const v: unknown = JSON.parse(raw);
		if (typeof v === 'object' && v !== null && (v as { expired?: unknown }).expired === true) {
			const { user_id } = v as { user_id?: unknown };
			return { userId: typeof user_id === 'number' ? user_id : null };
		}
	} catch {
		// not a mark
	}
	return undefined;
}

const STORAGES = [() => sessionStorage, () => localStorage];

/** What the storage of this tab holds now: its own (sessionStorage) first, then the shared one. */
function stored(): StoredSession | null {
	return parse(read(() => sessionStorage)) ?? parse(read(() => localStorage));
}

let held: StoredSession | null = typeof window === 'undefined' ? null : stored();
/** The session is in the storage (false: the storage would not take it, it lives in memory only). */
let kept = held !== null;
let onLost: (reason: LostReason, token: string) => void = () => {};
let onFound: () => void = () => {};

/** The token of this page's session; null: nobody is signed in here. */
export function getToken(): string | null {
	return held?.token ?? null;
}

/**
 * `remember` keeps the session between browser sessions, otherwise it lives until the tab closes. The
 * new value is written in one step: other tabs of the same user see a new token, not a sign-out.
 */
export function setToken(token: string, remember: boolean, userId: number | null): void {
	const value = JSON.stringify({ token, user_id: userId });
	held = { token, userId };
	try {
		if (remember) {
			// This tab's own copy (an earlier session without «remember») would be read first.
			remove(() => sessionStorage);
			localStorage.setItem(KEY, value);
		} else {
			sessionStorage.setItem(KEY, value);
			// The device remembers nobody else: another user's remembered session goes.
			const shared = parse(read(() => localStorage));
			if (shared !== null && (userId === null || shared.userId !== userId))
				remove(() => localStorage);
		}
		kept = true;
	} catch {
		// storage is unavailable: the user will have to sign in again after a reload
		kept = false;
	}
}

/** The user of this page's session, once `/auth/me` says it (a session stored by an earlier version). */
export function noteUser(userId: number | null): void {
	if (held !== null && held.userId === null) held = { ...held, userId };
}

/**
 * This page's session ends (signing out, an expired session): its token goes, from the storage too
 * while it is still there. A session another tab put there meanwhile stays: it is that tab's.
 */
export function clearToken(): void {
	const mine = held?.token;
	for (const storage of STORAGES)
		if (mine !== undefined && parse(read(storage))?.token === mine) remove(storage);
	held = null;
}

/**
 * This page's session expired (the server said 401): like `clearToken`, and the shared storage keeps a
 * mark of whose session it was, so that the other tabs say «expired», not «another user».
 */
export function expireToken(): void {
	const mine = held;
	// Only a session of the shared storage has other tabs to tell.
	if (mine !== null && parse(read(() => localStorage))?.token === mine.token) {
		// One write, not a removal and then a mark: a tab that heard the removal alone would say «changed».
		try {
			localStorage.setItem(KEY, JSON.stringify({ expired: true, user_id: mine.userId }));
		} catch {
			// storage is unavailable: the others will say «changed»
		}
	}
	clearToken();
}

/**
 * Follows what another tab did to the stored session; true when this page's session is over (another
 * user, or none). A new token of the same user is taken up here.
 */
export function followStorage(): boolean {
	if (held === null || !kept) return false;
	if (parse(read(() => sessionStorage))?.token === held.token) {
		// A tab of its own: only a known other user in the shared storage ends it.
		const shared = parse(read(() => localStorage));
		return (
			shared !== null &&
			shared.userId !== null &&
			held.userId !== null &&
			shared.userId !== held.userId
		);
	}
	const now = stored();
	if (now?.token === held.token) {
		if (held.userId === null && now.userId !== null) held = { ...held, userId: now.userId };
		return false;
	}
	if (now !== null && now.userId !== null && now.userId === held.userId) {
		held = now;
		return false;
	}
	return true;
}

/**
 * Ends this page's session after another tab ended it: the token is forgotten here (the storage is that
 * tab's now) and the program is told, once.
 */
export function loseSession(): void {
	if (held === null) return;
	const { token, userId } = held;
	let reason: LostReason = 'changed';
	if (stored() === null) {
		const mark = expiryMark();
		if (mark !== undefined && mark.userId === userId) reason = 'expired';
	}
	// A tab's own copy would be taken up again by the login page; the shared one is another tab's.
	if (parse(read(() => sessionStorage))?.token === token) remove(() => sessionStorage);
	held = null;
	onLost(reason, token);
}

/**
 * A page without a session takes the one another tab stored (the login page, when the user signed in
 * there). Null when there is none.
 */
export function adoptStored(): StoredSession | null {
	if (held !== null) return held;
	const now = stored();
	if (now === null) return null;
	held = now;
	kept = true;
	return now;
}

/** What the program does when another tab ended the session (the `(app)` layout sets it). */
export function setSessionLostHandler(fn: (reason: LostReason, token: string) => void): void {
	onLost = fn;
}

/** What a page without a session does when another tab stores one (the login page sets it). */
export function setSessionFoundHandler(fn: () => void): void {
	onFound = fn;
}

// Another tab changed the shared storage: this page follows at once, not only at its next request.
if (typeof window !== 'undefined')
	window.addEventListener('storage', (e) => {
		if (e.key !== KEY && e.key !== null) return;
		if (held === null) {
			if (stored() !== null) onFound();
			return;
		}
		if (followStorage()) loseSession();
	});
