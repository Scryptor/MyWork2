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

export function getToken(): string | null {
	return read(() => sessionStorage) ?? read(() => localStorage);
}

/** `remember` keeps the token between browser sessions, otherwise it lives until the tab closes. */
export function setToken(token: string, remember: boolean): void {
	clearToken();
	try {
		(remember ? localStorage : sessionStorage).setItem(KEY, token);
	} catch {
		// storage is unavailable: the user will have to sign in again after a reload
	}
}

export function clearToken(): void {
	remove(() => sessionStorage);
	remove(() => localStorage);
}
