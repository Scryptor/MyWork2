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

/** Fewer digits are a number being typed or erased, not a number: they are never saved. */
export const MIN_PHONE_DIGITS = 5;

/**
 * The list after typing `digits` into row `i` of `[...phones, draft]` (the draft: a row of «＋ номер»
 * not in the list yet). `before` is the row's number when typing in it began; null when the row was the
 * draft then. From MIN_PHONE_DIGITS digits on the number goes into the list; a shorter one (erased with
 * Backspace) leaves the number it had before typing, or the row of its own again.
 */
export function typedNumber(
	phones: readonly ClientPhone[],
	draft: ClientPhone | null,
	i: number,
	digits: string,
	before: string | null
): { phones: ClientPhone[]; draft: ClientPhone | null } {
	const ready = digits.length >= MIN_PHONE_DIGITS;
	const row = phones[i];
	if (row === undefined) {
		if (!draft) return { phones: [...phones], draft };
		if (!ready) return { phones: [...phones], draft: { ...draft, phone: digits } };
		return { phones: [...phones, { phone: digits, note: draft.note }], draft: null };
	}
	if (ready) return { phones: phones.with(i, { ...row, phone: digits }), draft };
	if (before === null) {
		return { phones: phones.toSpliced(i, 1), draft: { phone: digits, note: row.note } };
	}
	return { phones: phones.with(i, { ...row, phone: before }), draft };
}
