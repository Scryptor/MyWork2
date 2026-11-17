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
