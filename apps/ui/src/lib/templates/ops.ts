import type { Block } from '$api/Block';
import type { Template } from '$api/Template';
import { newBlock } from './defaults.js';
import { qrLinkId } from './qr.js';

/** `[i]` is a top-level block, `[i, c, j]` block `j` of column `c` of the columns block `i`. */
export type BlockPath = readonly number[];
/** The top level or a column: `[i, c]`. */
export type Container = readonly [] | readonly [number, number];

/** The list a path points into and the index in it; undefined when the container does not exist. */
function listOf(t: Template, path: BlockPath): { list: Block[]; index: number } | undefined {
	if (path.length === 1) return { list: t.blocks, index: path[0]! };
	if (path.length === 3) {
		const body = t.blocks[path[0]!]?.body;
		const list = body?.type === 'columns' ? body.columns[path[1]!] : undefined;
		return list && { list, index: path[2]! };
	}
	return undefined;
}

/** Applies `f` to the list of the container of a copy of the template. */
function edit(t: Template, path: BlockPath, f: (list: Block[], index: number) => void): Template {
	const next = structuredClone(t);
	const at = listOf(next, path);
	if (!at) return t;
	f(at.list, at.index);
	return next;
}

export function blockAt(t: Template, path: BlockPath): Block | undefined {
	const at = listOf(t, path);
	return at?.list[at.index];
}

export const replaceBlock = (t: Template, path: BlockPath, block: Block): Template =>
	edit(t, path, (list, i) => {
		if (i >= 0 && i < list.length) list[i] = structuredClone(block);
	});

/** `at` is the place of the new block. */
export const insertBlock = (t: Template, at: BlockPath, block: Block): Template =>
	edit(t, at, (list, i) => {
		list.splice(Math.max(0, Math.min(i, list.length)), 0, structuredClone(block));
	});

export const removeBlock = (t: Template, path: BlockPath): Template =>
	edit(t, path, (list, i) => {
		if (i >= 0 && i < list.length) list.splice(i, 1);
	});

/** A copy right after the original. */
export const duplicateBlock = (t: Template, path: BlockPath): Template =>
	edit(t, path, (list, i) => {
		if (i >= 0 && i < list.length) list.splice(i + 1, 0, structuredClone(list[i]!));
	});

/** A column holds neither columns nor a barcode that fills the page. */
export function canMove(t: Template, from: BlockPath, to: BlockPath): boolean {
	const block = blockAt(t, from);
	if (!block) return false;
	// The place must exist once the block is out of the way.
	if (!listOf(removeBlock(t, from), to)) return false;
	if (to.length === 3) {
		if (block.body.type === 'columns') return false;
		if (block.body.type === 'barcode' && block.body.fill) return false;
	}
	return true;
}

/**
 * Whether «Свой Typst-код» is offered for the block. A barcode is not: its module is counted in the
 * printer's dots by the barcode blocks, so it stays a block, and columns with one stay columns. A code
 * block in a column is written in place; one the server cannot write so it refuses (a 422).
 */
export function convertible(block: Block): boolean {
	const b = block.body;
	switch (b.type) {
		case 'barcode':
		case 'typst_code':
		case 'calibration_marks':
			return false;
		// The code of a firm-link block is a ready recipe; a link of its own the code cannot reproduce.
		case 'qr':
			return qrLinkId(b.link) !== null;
		case 'columns':
			return b.columns.every((column) =>
				column.every((inner) => inner.body.type === 'typst_code' || convertible(inner))
			);
		default:
			return true;
	}
}

/** `to` is the place in the resulting template. */
export function moveBlock(t: Template, from: BlockPath, to: BlockPath): Template {
	if (!canMove(t, from, to)) return t;
	return insertBlock(removeBlock(t, from), to, blockAt(t, from)!);
}

const isPermutation = (order: number[]) =>
	order.every((i) => Number.isInteger(i) && i >= 0 && i < order.length) &&
	new Set(order).size === order.length;

/** `order` lists the previous indices in the new order, as `SortableList` reports it. */
export function reorder(t: Template, container: Container, order: number[]): Template {
	const next = structuredClone(t);
	const list = container.length === 0 ? next.blocks : listOf(next, [...container, 0])?.list;
	if (!list || order.length !== list.length) return t;
	// Every previous index once: a repeated one would copy a block and lose another.
	if (!isPermutation(order)) return t;
	const old = [...list];
	const moved = order.map((i) => old[i]);
	list.splice(0, list.length, ...(moved as Block[]));
	return next;
}

/** The blocks in reading order with their paths; the blocks of a column follow their `columns` block. */
export function flatten(
	t: Template
): { path: BlockPath; block: Block; depth: 0 | 1; column?: number }[] {
	const out: { path: BlockPath; block: Block; depth: 0 | 1; column?: number }[] = [];
	t.blocks.forEach((block, i) => {
		out.push({ path: [i], block, depth: 0 });
		if (block.body.type === 'columns') {
			block.body.columns.forEach((col, c) =>
				col.forEach((inner, j) => out.push({ path: [i, c, j], block: inner, depth: 1, column: c }))
			);
		}
	});
	return out;
}

/** What is chosen in the list of blocks: the page or a block. */
export type Choice = BlockPath | 'page';

/** The chosen item after the block at `at` is gone: the neighbours after it move up. */
export function selectionAfterRemove(sel: Choice, at: BlockPath): Choice {
	if (sel === 'page') return sel;
	const depth = at.length - 1;
	const container = (p: BlockPath) => p.slice(0, depth).join('.');
	// The block itself, or a block of the columns that went with it.
	if (at.every((n, k) => sel[k] === n)) return 'page';
	if (sel.length <= depth || container(sel) !== container(at) || sel[depth]! < at[depth]!)
		return sel;
	return sel.map((n, k) => (k === depth ? n - 1 : n));
}

/** The copy made by `duplicateBlock` is the one chosen. */
export const selectionAfterDuplicate = (path: BlockPath): BlockPath =>
	path.map((n, k) => (k === path.length - 1 ? n + 1 : n));

/** Where a new block goes: after the chosen one, in its column unless it is columns itself. */
export function placeForNew(t: Template, sel: Choice, type: Block['body']['type']): BlockPath {
	if (sel === 'page') return [t.blocks.length];
	if (sel.length === 3 && type !== 'columns') return [sel[0]!, sel[1]!, sel[2]! + 1];
	return [sel[0]! + 1];
}

/**
 * «В колонку N» of a top-level block: the end of every column it may go into. `block` is the place of
 * the columns block in the template; `to` counts in the template without the block moved.
 */
export function intoColumnTargets(
	t: Template,
	path: BlockPath
): { column: number; block: number; to: BlockPath }[] {
	if (path.length !== 1) return [];
	const from = path[0]!;
	return t.blocks.flatMap((b, i) => {
		if (b.body.type !== 'columns') return [];
		return b.body.columns.flatMap((column, c) => {
			const to = [i > from ? i - 1 : i, c, column.length];
			return canMove(t, path, to) ? [{ column: c, block: i, to }] : [];
		});
	});
}

/**
 * The text of an old MyWork 2 act as one «Текст» block with the starting style, put where a new text
 * block goes (after the chosen block, or at the end). `at` is its place, to choose it afterwards.
 */
export function insertTextBlock(
	t: Template,
	sel: Choice,
	text: string
): { template: Template; at: BlockPath } {
	const block = newBlock('text');
	if (block.body.type === 'text') block.body.text = text;
	const at = placeForNew(t, sel, 'text');
	return { template: insertBlock(t, at, block), at };
}
