// Saving a drag order when somebody else has changed the list meanwhile. The server takes only an exact list of
// all the ids, together with the order it was made from: a list with other ids, or an order another screen has
// changed since, is refused. Then the list is read again, the move is made again on the fresh list (the moved
// id next to the same neighbour, the rest where the server has them — so the other move is kept) and sent once
// more.
import { ApiError } from '#lib/api/client.js';

/** The one id whose move turns `before` into `after`, or `null` when it is not a single move. */
export function movedId(before: number[], after: number[]): number | null {
	if (before.length !== after.length) return null;
	for (const id of before) {
		const a = before.filter((x) => x !== id);
		const b = after.filter((x) => x !== id);
		if (a.length === b.length && a.every((x, i) => x === b[i])) return id;
	}
	return null;
}

/** `fresh` with `moved` put next to the neighbour it has in `intended`: after its predecessor, else before its successor. */
export function reapplyMove(fresh: number[], intended: number[], moved: number): number[] {
	const rest = fresh.filter((x) => x !== moved);
	const at = intended.indexOf(moved);
	const known = new Set(rest);
	const prev = intended
		.slice(0, at)
		.reverse()
		.find((x) => known.has(x));
	if (prev !== undefined) {
		rest.splice(rest.indexOf(prev) + 1, 0, moved);
		return rest;
	}
	const next = intended.slice(at + 1).find((x) => known.has(x));
	if (next !== undefined) rest.splice(rest.indexOf(next), 0, moved);
	else rest.push(moved);
	return rest;
}

/** The server said the order it has is not the one the move was made from (`ordered_ids: stale`). */
const isStale = (e: unknown) =>
	e instanceof ApiError && (e.body?.fields.some((f) => f.code === 'stale') ?? false);

/**
 * Sends `ordered`, made from `seen`; when the server refuses and its list is not `seen` any more (other ids, or
 * another order), or it says the order was stale (it may have settled by the time it is read), sends the move
 * again on the fresh list. Answers the order that was saved; throws the refusal when it was not about that.
 */
export async function putOrder(opts: {
	ordered: number[];
	seen: number[];
	moved: number | null;
	put: (ids: number[], seen: number[]) => Promise<void>;
	fresh: () => Promise<number[]>;
}): Promise<number[]> {
	try {
		await opts.put(opts.ordered, opts.seen);
		return opts.ordered;
	} catch (first) {
		if (opts.moved === null) throw first;
		let fresh: number[];
		try {
			fresh = await opts.fresh();
		} catch {
			throw first;
		}
		const same = fresh.length === opts.seen.length && fresh.every((x, i) => x === opts.seen[i]);
		if (same && !isStale(first)) throw first;
		const again = reapplyMove(fresh, opts.ordered, opts.moved);
		await opts.put(again, fresh);
		return again;
	}
}
