export type StorageArea = 'local' | 'session';

function area(kind: StorageArea): Storage {
	return kind === 'session' ? sessionStorage : localStorage;
}

/** The stored text, or null when there is none or storage is unavailable (private mode, blocked). */
export function readStorage(key: string, kind: StorageArea = 'local'): string | null {
	try {
		return area(kind).getItem(key);
	} catch {
		return null;
	}
}

/** Stores the text; when storage is unavailable the app keeps working with what it has in memory. */
export function writeStorage(key: string, value: string, kind: StorageArea = 'local'): void {
	try {
		area(kind).setItem(key, value);
	} catch {
		// storage unavailable
	}
}

export function removeStorage(key: string, kind: StorageArea = 'local'): void {
	try {
		area(kind).removeItem(key);
	} catch {
		// storage unavailable
	}
}
