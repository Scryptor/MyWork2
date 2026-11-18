// The phones of a client (spec 6.6): one list, the first number is the main one; each has a note.
import type { ClientPhone } from '$api/ClientPhone';
import { phoneDigits } from '#lib/format/phone.js';
import { sameValue } from '#lib/orders/merge.js';

/**
 * The list as the server keeps it: rows with a number, digits only, notes trimmed. A row without a
 * number is not sent: the server would refuse a note without one.
 */
export function savedPhones(rows: readonly ClientPhone[]): ClientPhone[] {
	return rows.flatMap((p) => {
		const phone = phoneDigits(p.phone);
		return phone === '' ? [] : [{ phone, note: p.note.trim() }];
	});
}

/** The server kept a list sent in its own form only: the field may show it as typed. */
export function keptPhones(typed: unknown, server: unknown): boolean {
	return Array.isArray(typed) && sameValue(savedPhones(typed as ClientPhone[]), server);
}

/** «Сделать основным»: this number goes first, the others keep their order. */
export function asPrimary(rows: readonly ClientPhone[], i: number): ClientPhone[] {
	const row = rows[i];
	if (i <= 0 || row === undefined) return [...rows];
	return [row, ...rows.slice(0, i), ...rows.slice(i + 1)];
}
