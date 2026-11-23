import { describe, expect, it } from 'vitest';
import type { Block } from '$api/Block';
import type { Template } from '$api/Template';
import {
	blockAt,
	canMove,
	duplicateBlock,
	flatten,
	insertBlock,
	insertTextBlock,
	intoColumnTargets,
	moveBlock,
	placeForNew,
	selectionAfterDuplicate,
	selectionAfterRemove,
	removeBlock,
	reorder,
	replaceBlock
} from './ops.js';

const text = (s: string): Block => ({
	copies: 'both',
	body: { type: 'text', text: s, size_pt: null, bold: false, align: 'left', one_line: false }
});
const columns = (a: Block[], b: Block[]): Block => ({
	copies: 'both',
	body: { type: 'columns', columns: [a, b], gap_mm: 6 }
});
const fillBarcode: Block = {
	copies: 'both',
	body: {
		type: 'barcode',
		height_mm: 12,
		digits: true,
		align: 'center',
		fill: true,
		stretch: false
	}
};
const tpl = (blocks: Block[]): Template => ({
	kind: 'intake_act',
	page: {} as Template['page'],
	layout: {} as Template['layout'],
	font_size_pt: 10,
	blocks
});
const label = (b: Block | undefined) =>
	b?.body.type === 'text' ? b.body.text : (b?.body.type ?? '-');
const tops = (t: Template) => t.blocks.map(label);
const col = (t: Template, i: number, c: number) => {
	const b = t.blocks[i]!.body;
	return b.type === 'columns' ? b.columns[c]!.map(label) : [];
};

/** Runs `f` on a template and checks that the original did not change. */
function pure<T>(t: Template, f: (t: Template) => T): T {
	const before = structuredClone(t);
	const r = f(t);
	expect(t).toEqual(before);
	return r;
}

const base = () => tpl([text('a'), columns([text('x'), text('y')], [text('z')]), text('b')]);

describe('ops', () => {
	it('blockAt at the top and in a column', () => {
		const t = base();
		expect(label(blockAt(t, [0]))).toBe('a');
		expect(label(blockAt(t, [1, 0, 1]))).toBe('y');
		expect(blockAt(t, [5])).toBeUndefined();
		expect(blockAt(t, [1, 3, 0])).toBeUndefined();
		expect(blockAt(t, [0, 0, 0])).toBeUndefined();
	});

	it('replaceBlock', () => {
		const t = base();
		expect(tops(pure(t, (x) => replaceBlock(x, [0], text('A'))))).toEqual(['A', 'columns', 'b']);
		expect(
			col(
				pure(t, (x) => replaceBlock(x, [1, 0, 1], text('Y'))),
				1,
				0
			)
		).toEqual(['x', 'Y']);
	});

	it('insertBlock puts the block at the place', () => {
		const t = base();
		expect(tops(pure(t, (x) => insertBlock(x, [1], text('n'))))).toEqual([
			'a',
			'n',
			'columns',
			'b'
		]);
		expect(tops(pure(t, (x) => insertBlock(x, [3], text('n'))))).toEqual([
			'a',
			'columns',
			'b',
			'n'
		]);
		expect(
			col(
				pure(t, (x) => insertBlock(x, [1, 1, 0], text('n'))),
				1,
				1
			)
		).toEqual(['n', 'z']);
		expect(
			col(
				pure(t, (x) => insertBlock(x, [1, 1, 1], text('n'))),
				1,
				1
			)
		).toEqual(['z', 'n']);
	});

	it('removeBlock', () => {
		const t = base();
		expect(tops(pure(t, (x) => removeBlock(x, [0])))).toEqual(['columns', 'b']);
		expect(
			col(
				pure(t, (x) => removeBlock(x, [1, 0, 0])),
				1,
				0
			)
		).toEqual(['y']);
	});

	it('duplicateBlock puts a deep copy right after', () => {
		const t = base();
		const d = pure(t, (x) => duplicateBlock(x, [0]));
		expect(tops(d)).toEqual(['a', 'a', 'columns', 'b']);
		expect(d.blocks[1]).not.toBe(d.blocks[0]);
		expect(
			col(
				pure(t, (x) => duplicateBlock(x, [1, 0, 0])),
				1,
				0
			)
		).toEqual(['x', 'x', 'y']);
		const c = duplicateBlock(t, [1]);
		expect(c.blocks[2]).toEqual(c.blocks[1]);
		expect(c.blocks[2]).not.toBe(c.blocks[1]);
	});

	it('canMove: allowed and forbidden moves', () => {
		const t = tpl([text('a'), columns([text('x')], []), fillBarcode, columns([], [])]);
		expect(canMove(t, [0], [0, 1, 0])).toBe(true);
		expect(canMove(t, [3], [1, 0, 0])).toBe(false);
		expect(canMove(t, [2], [1, 0, 0])).toBe(false);
		expect(canMove(t, [1, 0, 0], [0])).toBe(true);
		expect(canMove(t, [2], [0])).toBe(true);
		expect(canMove(t, [9], [0])).toBe(false);
	});

	it('moveBlock: to is the place in the result', () => {
		const t = base();
		expect(tops(pure(t, (x) => moveBlock(x, [0], [2])))).toEqual(['columns', 'b', 'a']);
		expect(tops(pure(t, (x) => moveBlock(x, [2], [0])))).toEqual(['b', 'a', 'columns']);
		const into = pure(t, (x) => moveBlock(x, [0], [0, 1, 0])); // column of the block that shifts to 0
		expect(col(into, 0, 1)).toEqual(['a', 'z']);
		expect(tops(into)).toEqual(['columns', 'b']);
		const out = pure(t, (x) => moveBlock(x, [1, 0, 0], [0]));
		expect(tops(out)).toEqual(['x', 'a', 'columns', 'b']);
		const sameCol = pure(t, (x) => moveBlock(x, [1, 0, 0], [1, 0, 1]));
		expect(col(sameCol, 1, 0)).toEqual(['y', 'x']);
	});

	it('moveBlock returns the template as is when canMove is false', () => {
		const t = tpl([columns([], []), columns([], [])]);
		expect(moveBlock(t, [0], [1, 0, 0])).toBe(t);
	});

	it('reorder takes the previous indices in the new order', () => {
		const t = tpl([text('a'), text('b'), text('c')]);
		expect(tops(pure(t, (x) => reorder(x, [], [2, 0, 1])))).toEqual(['c', 'a', 'b']);
		const c = tpl([columns([text('p'), text('q'), text('r')], [text('z')])]);
		expect(
			col(
				pure(c, (x) => reorder(x, [0, 0], [2, 0, 1])),
				0,
				0
			)
		).toEqual(['r', 'p', 'q']);
		expect(col(reorder(c, [0, 0], [2, 0, 1]), 0, 1)).toEqual(['z']);
	});

	it('flatten lists blocks with paths, depth and column', () => {
		const t = base();
		const f = pure(t, flatten);
		expect(f.map((e) => [e.path, e.depth, e.column])).toEqual([
			[[0], 0, undefined],
			[[1], 0, undefined],
			[[1, 0, 0], 1, 0],
			[[1, 0, 1], 1, 0],
			[[1, 1, 0], 1, 1],
			[[2], 0, undefined]
		]);
		expect(f[3]!.block).toBe(
			t.blocks[1]!.body.type === 'columns' ? t.blocks[1]!.body.columns[0]![1] : null
		);
	});
});

describe('selectionAfterRemove', () => {
	it('the page stays the page', () => {
		expect(selectionAfterRemove('page', [1])).toBe('page');
	});
	it('the block removed leaves the page', () => {
		expect(selectionAfterRemove([2], [2])).toBe('page');
	});
	it('a block of the columns removed with them leaves the page', () => {
		expect(selectionAfterRemove([2, 0, 1], [2])).toBe('page');
	});
	it('later blocks of the same list move up', () => {
		expect(selectionAfterRemove([3], [1])).toEqual([2]);
		expect(selectionAfterRemove([1, 1, 2], [1, 1, 0])).toEqual([1, 1, 1]);
	});
	it('earlier blocks and other lists stay', () => {
		expect(selectionAfterRemove([0], [1])).toEqual([0]);
		expect(selectionAfterRemove([1, 1, 2], [1, 0, 0])).toEqual([1, 1, 2]);
	});
	it('a top block removed before columns moves their blocks up', () => {
		expect(selectionAfterRemove([2, 0, 1], [0])).toEqual([1, 0, 1]);
	});
});

describe('selectionAfterDuplicate', () => {
	it('the copy is the next one in the list', () => {
		expect(selectionAfterDuplicate([1])).toEqual([2]);
		expect(selectionAfterDuplicate([1, 0, 3])).toEqual([1, 0, 4]);
	});
});

describe('placeForNew', () => {
	const t = tpl([text('a'), columns([text('x')], []), text('b')]);
	it('the end of the page without a choice', () => {
		expect(placeForNew(t, 'page', 'text')).toEqual([3]);
	});
	it('after the chosen top block', () => {
		expect(placeForNew(t, [0], 'text')).toEqual([1]);
	});
	it('after the chosen block of a column, in that column', () => {
		expect(placeForNew(t, [1, 0, 0], 'text')).toEqual([1, 0, 1]);
	});
	it('columns never go into a column: after the block of columns', () => {
		expect(placeForNew(t, [1, 0, 0], 'columns')).toEqual([2]);
	});
});

describe('intoColumnTargets', () => {
	it('the end of every column; the index counts the template without the block moved', () => {
		const t = tpl([text('a'), columns([text('x')], []), text('b')]);
		expect(intoColumnTargets(t, [0])).toEqual([
			{ column: 0, block: 1, to: [0, 0, 1] },
			{ column: 1, block: 1, to: [0, 1, 0] }
		]);
		expect(intoColumnTargets(t, [2])).toEqual([
			{ column: 0, block: 1, to: [1, 0, 1] },
			{ column: 1, block: 1, to: [1, 1, 0] }
		]);
	});
	it('nothing for a block of a column, for columns, for a filled barcode', () => {
		const t = tpl([columns([text('x')], []), columns([], []), fillBarcode]);
		expect(intoColumnTargets(t, [0, 0, 0])).toEqual([]);
		expect(intoColumnTargets(t, [0])).toEqual([]);
		expect(intoColumnTargets(t, [2])).toEqual([]);
	});
	it('the moves it offers are the moves canMove allows', () => {
		const t = tpl([text('a'), columns([], []), text('b'), columns([text('y')], [])]);
		for (const target of intoColumnTargets(t, [0])) expect(canMove(t, [0], target.to)).toBe(true);
	});

	it('insertTextBlock puts a text block with the starting style after the chosen block', () => {
		const { template, at } = pure(base(), (t) =>
			insertTextBlock(t, [0], 'Старый акт\n{заказ.номер}')
		);
		expect(at).toEqual([1]);
		expect(tops(template)).toEqual(['a', 'Старый акт\n{заказ.номер}', 'columns', 'b']);
		expect(template.blocks[1]).toEqual({
			copies: 'both',
			body: {
				type: 'text',
				text: 'Старый акт\n{заказ.номер}',
				size_pt: null,
				bold: false,
				align: 'left',
				one_line: false
			}
		});
	});

	it('insertTextBlock without a chosen block goes to the end; in a column, after the block', () => {
		expect(tops(insertTextBlock(base(), 'page', 'T').template)).toEqual(['a', 'columns', 'b', 'T']);
		const inColumn = insertTextBlock(base(), [1, 0, 0], 'T');
		expect(inColumn.at).toEqual([1, 0, 1]);
		expect(col(inColumn.template, 1, 0)).toEqual(['x', 'T', 'y']);
	});
});
