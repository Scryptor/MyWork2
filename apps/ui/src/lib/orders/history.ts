import type { AuditEntry } from '$api/AuditEntry';

/** History lines for `Timeline`: the server writes the text of each entry in Russian. */
export function historyItems(entries: readonly AuditEntry[]) {
	return entries.map((e) => ({ id: e.id, at: e.at, who: e.user_name, text: e.summary }));
}

function statusMove(e: AuditEntry): { from: number; to: number } | null {
	if (e.action !== 'order.status_changed') return null;
	const d = e.details;
	if (typeof d !== 'object' || d === null || Array.isArray(d)) return null;
	const { from, to } = d as Record<string, unknown>;
	return typeof from === 'number' && typeof to === 'number' ? { from, to } : null;
}

/**
 * «Вернуть в работу»: the status the order had before it got the current one — the newest
 * `order.status_changed` into it. Null when there is none or that status cannot be chosen now.
 * `entries` come newest first, as the server gives them.
 */
export function statusBeforeIssue(
	entries: readonly AuditEntry[],
	current: number,
	choices: readonly number[]
): number | null {
	for (const e of entries) {
		const move = statusMove(e);
		if (move?.to === current) return choices.includes(move.from) ? move.from : null;
	}
	return null;
}

/** Entries of «Статусы и сроки»: the intake, every status the order got, warranty cases. */
const TIMELINE_ACTIONS = [
	'order.created',
	'order.status_changed',
	'legacy.status',
	'order.warranty_case'
];

export function statusTimeline(entries: readonly AuditEntry[]) {
	return historyItems(entries.filter((e) => TIMELINE_ACTIONS.includes(e.action)));
}

export interface WarrantyCaseItem {
	id: number;
	/** When the client came back; the time of the entry when the details do not say. */
	at: string;
	complaint: string;
}

/** «Гарантийные случаи»: the date and the complaint of each `order.warranty_case`. */
export function warrantyCases(entries: readonly AuditEntry[]): WarrantyCaseItem[] {
	return entries
		.filter((e) => e.action === 'order.warranty_case')
		.map((e) => {
			const d = e.details;
			const fields =
				typeof d === 'object' && d !== null && !Array.isArray(d)
					? (d as Record<string, unknown>)
					: {};
			return {
				id: e.id,
				at: typeof fields.at === 'string' ? fields.at : e.at,
				complaint: typeof fields.complaint === 'string' ? fields.complaint : e.summary
			};
		});
}

/** A fresh first page over the entries shown: new ones on top, each entry once, newest first. */
export function mergeHistory(
	shown: readonly AuditEntry[],
	fresh: readonly AuditEntry[]
): AuditEntry[] {
	const byId = new Map(shown.map((e) => [e.id, e]));
	for (const e of fresh) byId.set(e.id, e);
	return [...byId.values()].sort((a, b) => b.id - a.id);
}

/**
 * The cursor after the oldest entry in the order the server shows them (time, then id, both falling):
 * the entries shown are a prefix of that order, so it is the smallest by (time, id). The time is a
 * local time without offset, it compares as text.
 */
export function historyCursor(
	entries: readonly AuditEntry[]
): { before_at: string; before_id: number } | undefined {
	let last: AuditEntry | undefined;
	for (const e of entries) {
		if (!last || e.at < last.at || (e.at === last.at && e.id < last.id)) last = e;
	}
	return last && { before_at: last.at, before_id: last.id };
}
