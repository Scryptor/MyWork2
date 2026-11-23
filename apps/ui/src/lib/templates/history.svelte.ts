/** An undo history of immutable values; the value is reactive. */
export function createHistory<T>(initial: T, opts: { limit?: number; mergeMs?: number } = {}) {
	const limit = opts.limit ?? 100;
	const mergeMs = opts.mergeMs ?? 1000;
	// `entries` holds the values, `index` the current one; the older ones are the undo steps.
	let entries = $state.raw<T[]>([initial]);
	let index = $state(0);
	let lastKey: string | undefined;
	let lastAt = 0;

	return {
		get current(): T {
			return entries[index]!;
		},
		get canUndo(): boolean {
			return index > 0;
		},
		get canRedo(): boolean {
			return index < entries.length - 1;
		},
		/** A push with the key of the previous one within `mergeMs` replaces the top entry. */
		push(next: T, mergeKey?: string): void {
			const now = Date.now();
			const merge =
				mergeKey !== undefined &&
				mergeKey === lastKey &&
				now - lastAt <= mergeMs &&
				index === entries.length - 1 &&
				index > 0;
			const kept = entries.slice(0, merge ? index : index + 1);
			kept.push(next);
			// The initial value counts as an entry: `limit` undo steps need `limit + 1` of them.
			if (kept.length > limit + 1) kept.splice(0, kept.length - limit - 1);
			entries = kept;
			index = kept.length - 1;
			lastKey = mergeKey;
			lastAt = now;
		},
		undo(): T | undefined {
			if (index === 0) return undefined;
			lastKey = undefined;
			index -= 1;
			return entries[index];
		},
		redo(): T | undefined {
			if (index >= entries.length - 1) return undefined;
			lastKey = undefined;
			index += 1;
			return entries[index];
		},
		reset(to: T): void {
			entries = [to];
			index = 0;
			lastKey = undefined;
		}
	};
}
