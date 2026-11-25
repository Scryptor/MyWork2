// The firm's links printed as QR codes (plan 8.3): the built-in ones cannot be deleted, an own link gets
// its id here and the server only checks its form and that it is unique.
import type { FirmLink } from '$api/FirmLink';

/** The built-in links, in their order: renamed or switched off, never deleted. */
export const BUILTIN_IDS = ['yandex_maps', '2gis', 'telegram', 'max'] as const;

/** At most this many links in the list (as on the server). */
export const MAX_LINKS = 20;

export function isBuiltin(id: string): boolean {
	return (BUILTIN_IDS as readonly string[]).includes(id);
}

/** `link_` and the time in base 36; a counter is added when the id is taken. */
export function newLinkId(existing: readonly FirmLink[]): string {
	const taken = new Set(existing.map((l) => l.id));
	const base = `link_${Date.now().toString(36)}`;
	let id = base;
	for (let n = 2; taken.has(id); n += 1) id = `${base}_${n}`;
	return id;
}

/** «Новая ссылка», «Новая ссылка 2», …: the first name that no link has (the server compares without case). */
export function newLinkName(existing: readonly FirmLink[], base: string): string {
	const taken = new Set(existing.map((l) => l.name.trim().toLowerCase()));
	let name = base;
	for (let n = 2; taken.has(name.toLowerCase()); n += 1) name = `${base} ${n}`;
	return name;
}
