import { readStorage, writeStorage } from '#lib/design/storage.js';

/** Orders opened on this device, newest first: «Недавние» of Ctrl K. */
export const RECENT_KEY = 'mw.recent';
const MAX_RECENT = 8;

export function recentIds(): number[] {
	let raw: unknown;
	try {
		raw = JSON.parse(readStorage(RECENT_KEY) ?? '[]');
	} catch {
		return [];
	}
	if (!Array.isArray(raw)) return [];
	const ids = raw.filter((x): x is number => Number.isInteger(x) && x > 0);
	return [...new Set(ids)].slice(0, MAX_RECENT);
}

export function pushRecent(id: number): void {
	const ids = [id, ...recentIds().filter((x) => x !== id)].slice(0, MAX_RECENT);
	writeStorage(RECENT_KEY, JSON.stringify(ids));
}
