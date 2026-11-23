// The keys of the rows of the list of blocks (plan 7.1). The model has no ids and every operation makes
// a new copy of it, so the keys live beside the templates of the history: a reorder takes the keys along
// with their blocks, an undo or a redo brings back the keys of its step, and any other edit keeps the
// keys of a list whose length stayed (it changed blocks in their places). The keys are unique in the
// whole list, the columns too: a handle is found by its key.
import type { Template } from '$api/Template';
import type { Container } from './ops.js';

export interface BlockKeys {
	/** The keys of the top-level blocks, in their order. */
	top: number[];
	/** By the place on the page: the keys of each column of a columns block; null for other blocks. */
	columns: (number[][] | null)[];
}

export function createBlockKeys() {
	const known = new WeakMap<Template, BlockKeys>();
	let next = 1;
	/** The keys of the template shown last: a new template takes them over where it has the same shape. */
	let last: BlockKeys | null = null;

	const fresh = (n: number) => Array.from({ length: n }, () => next++);
	const keep = (old: number[] | undefined, n: number) =>
		old !== undefined && old.length === n ? old : fresh(n);

	function inherit(t: Template, from: BlockKeys | null): BlockKeys {
		const same = from !== null && from.top.length === t.blocks.length;
		return {
			top: same ? from.top : fresh(t.blocks.length),
			columns: t.blocks.map((b, i) => {
				if (b.body.type !== 'columns') return null;
				const old = same ? from.columns[i] : null;
				return b.body.columns.map((column, c) => keep(old?.[c], column.length));
			})
		};
	}

	function of(t: Template): BlockKeys {
		let keys = known.get(t);
		if (keys === undefined) {
			keys = inherit(t, last);
			known.set(t, keys);
		}
		last = keys;
		return keys;
	}

	return {
		/** The keys of a template; the same template always has the same keys. */
		of,
		/** The keys of the list of `container`: the top level or a column. */
		listOf(keys: BlockKeys, container: Container): number[] | undefined {
			return container.length === 0 ? keys.top : keys.columns[container[0]]?.[container[1]];
		},
		/** `after` is `before` with the list of `container` in `order` (its keys in the new order). */
		reordered(before: Template, after: Template, container: Container, order: number[]): void {
			const keys = of(before);
			if (container.length === 0) {
				const columns = order.map((k) => keys.columns[keys.top.indexOf(k)] ?? null);
				known.set(after, { top: order, columns });
				return;
			}
			const [i, c] = container;
			const columns = keys.columns.map((cols, at) =>
				at === i && cols ? cols.map((list, k) => (k === c ? order : list)) : cols
			);
			known.set(after, { top: keys.top, columns });
		}
	};
}
