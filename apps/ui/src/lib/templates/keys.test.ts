import { describe, expect, it } from 'vitest';
import type { Block } from '$api/Block';
import type { Template } from '$api/Template';
import { createBlockKeys } from './keys.js';
import { insertBlock, reorder, replaceBlock } from './ops.js';

const text = (s: string): Block => ({
	copies: 'both',
	body: { type: 'text', text: s, size_pt: null, bold: false, align: 'left', one_line: false }
});
const columns: Block = {
	copies: 'both',
	body: { type: 'columns', gap_mm: 4, columns: [[text('а'), text('б')], [text('в')]] }
};
const T: Template = {
	kind: 'warranty_intake_act',
	page: { size: 'a4', landscape: false, margins_mm: { top: 10, right: 10, bottom: 10, left: 10 } },
	layout: 'two_per_a4',
	font_size_pt: 9,
	blocks: [text('Шапка'), columns, text('Подвал')]
};

describe('createBlockKeys', () => {
	it('gives every row its own key, and the same template the same keys', () => {
		const keys = createBlockKeys();
		const k = keys.of(T);
		const all = [...k.top, ...k.columns.flatMap((c) => c?.flat() ?? [])];
		expect(new Set(all).size).toBe(6);
		expect(k.columns[0]).toBeNull();
		expect(keys.of(T)).toBe(k);
	});

	it('an edit in place keeps the keys; a new block makes new ones', () => {
		const keys = createBlockKeys();
		const k = keys.of(T);
		expect(keys.of(replaceBlock(T, [0], text('Новая шапка')))).toEqual(k);
		const added = keys.of(insertBlock(T, [1], text('Ещё')));
		expect(added.top).toHaveLength(4);
		expect(added.top.some((key) => k.top.includes(key))).toBe(false);
	});

	it('a reorder takes the keys along, the columns of a moved columns block too; undo brings back its own', () => {
		const keys = createBlockKeys();
		const k = keys.of(T);
		const order = [k.top[1]!, k.top[0]!, k.top[2]!];
		const moved = reorder(T, [], [1, 0, 2]);
		keys.reordered(T, moved, [], order);
		expect(keys.of(moved)).toEqual({ top: order, columns: [k.columns[1], null, null] });

		const inColumn = reorder(moved, [0, 0], [1, 0]);
		const col = k.columns[1]![0]!;
		keys.reordered(moved, inColumn, [0, 0], [col[1]!, col[0]!]);
		expect(keys.of(inColumn).columns[0]).toEqual([[col[1], col[0]], k.columns[1]![1]]);
		// An undo shows the template before: its keys are the ones it had.
		expect(keys.of(T)).toBe(k);
	});
});
