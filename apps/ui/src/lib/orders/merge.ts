/** Equal values: plain ones by value, objects and arrays (a warranty term) by their contents. */
export function sameValue(a: unknown, b: unknown): boolean {
	if (Object.is(a, b)) return true;
	if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
	if (Array.isArray(a) !== Array.isArray(b)) return false;
	const ka = Object.keys(a);
	const kb = Object.keys(b);
	return (
		ka.length === kb.length &&
		ka.every((k) => sameValue((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]))
	);
}

/**
 * A record changed elsewhere while it was being edited here (spec 6.2, «Заказ изменили на другом
 * компьютере»). `base` is the record this side last had from the server, `local` the changes not saved
 * yet, `remote` the record as it is now.
 * - a field only the other side changed is taken (`take`, `changedByOthers`);
 * - a field both changed to different values is a conflict;
 * - the same change on both sides is no conflict.
 */
export function mergeRemote<T extends object>(
	base: T,
	local: Partial<T>,
	remote: T
): { take: Partial<T>; conflicts: (keyof T)[]; changedByOthers: (keyof T)[] } {
	const take: Partial<T> = {};
	const conflicts: (keyof T)[] = [];
	const changedByOthers: (keyof T)[] = [];
	for (const key of Object.keys(remote) as (keyof T)[]) {
		if (sameValue(base[key], remote[key])) continue;
		if (key in local) {
			if (!sameValue(local[key], remote[key])) conflicts.push(key);
			continue;
		}
		take[key] = remote[key];
		changedByOthers.push(key);
	}
	return { take, conflicts, changedByOthers };
}
