// Saving a drag order when somebody else has added a value meanwhile. The server takes only an exact list of
// all the ids, so a stale list is refused; then the list is read again, the move is made again on the fresh
// list (the moved id next to the same neighbour, unknown ids stay where the server has them) and sent once more.

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

/**
 * Sends `ordered`; when the server refuses and the list of the server has other ids than we knew, sends the
 * move again on the fresh list. Answers the order that was saved; throws the refusal when it was not about that.
 */
export async function putOrder(opts: {
	ordered: number[];
	moved: number | null;
	put: (ids: number[]) => Promise<void>;
	fresh: () => Promise<number[]>;
}): Promise<number[]> {
	try {
		await opts.put(opts.ordered);
		return opts.ordered;
	} catch (first) {
		if (opts.moved === null) throw first;
		let fresh: number[];
		try {
			fresh = await opts.fresh();
		} catch {
			throw first;
		}
		const known = new Set(opts.ordered);
		if (fresh.length === opts.ordered.length && fresh.every((x) => known.has(x))) throw first;
		const again = reapplyMove(fresh, opts.ordered, opts.moved);
		await opts.put(again);
		return again;
	}
}
