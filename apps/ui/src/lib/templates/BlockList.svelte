<!-- The list of blocks of the editor (plan 7.1): «Страница» first, then the blocks in reading order; the
     blocks of a column under it, «Колонка 1», «Колонка 2»…. A block is dragged within its own list (the
     page or its column); its menu duplicates, deletes and moves it into a column or out of the columns,
     and turns it into own Typst code that prints the same (plan 7.3).
     «Добавить блок» puts the new block after the chosen one and chooses it. -->
<script lang="ts">
	import ArrowLeftFromLine from '@lucide/svelte/icons/arrow-left-from-line';
	import ArrowRightToLine from '@lucide/svelte/icons/arrow-right-to-line';
	import Code from '@lucide/svelte/icons/code';
	import Copy from '@lucide/svelte/icons/copy';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import FileText from '@lucide/svelte/icons/file-text';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import type { Block } from '$api/Block';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { blockCode } from '#lib/api/document-templates.js';
	import { ru } from '#lib/i18n/ru.js';
	import { sameValue } from '#lib/orders/merge.js';
	import { untrack } from 'svelte';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import SortableList from '#lib/ui/SortableList.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import { BLOCK_TYPES, newBlock } from './defaults.js';
	import { blockLabel, type Selection, type TemplateEditor } from './editor.svelte.js';
	import { createBlockKeys } from './keys.js';
	import {
		blockAt,
		canMove,
		convertible,
		duplicateBlock,
		insertBlock,
		intoColumnTargets,
		moveBlock,
		placeForNew,
		removeBlock,
		reorder,
		replaceBlock,
		selectionAfterDuplicate,
		selectionAfterRemove,
		type BlockPath,
		type Container
	} from './ops.js';

	let { editor }: { editor: TemplateEditor } = $props();

	const t = ru.templates;

	/** A row of a list: the block, its path, its place in its own list and the key of its row. */
	interface Entry {
		block: Block;
		path: BlockPath;
		index: number;
		key: number;
	}

	const template = $derived(editor.template);
	const selected = $derived(editor.selected);
	// The rows follow their blocks: a handle moved with the arrows keeps the focus on its block.
	const blockKeys = createBlockKeys();
	// The keys are set by an explicit update: `of` remembers the template it was asked for, which is a
	// write that does not belong in a `$derived`.
	let keys = $state.raw(blockKeys.of(untrack(() => editor.template)));
	$effect.pre(() => {
		keys = blockKeys.of(editor.template);
	});
	const tops = $derived<Entry[]>(
		template.blocks.map((block, i) => ({ block, path: [i], index: i, key: keys.top[i]! }))
	);
	const columnEntries = (i: number, c: number, column: Block[]): Entry[] =>
		column.map((block, j) => ({ block, path: [i, c, j], index: j, key: keys.columns[i]![c]![j]! }));
	/** The blocks of columns, by their place on the page. */
	const columnBlocks = $derived(
		template.blocks.flatMap((b, i) => (b.body.type === 'columns' ? [{ i, body: b.body }] : []))
	);

	const same = (a: Selection, b: BlockPath) =>
		a !== 'page' && a.length === b.length && a.every((n, k) => n === b[k]);

	function remove(path: BlockPath) {
		const sel = selectionAfterRemove(selected, path);
		editor.edit(removeBlock(template, path));
		editor.select(sel);
	}

	function duplicate(path: BlockPath) {
		editor.edit(duplicateBlock(template, path));
		editor.select(selectionAfterDuplicate(path));
	}

	function move(from: BlockPath, to: BlockPath) {
		if (!canMove(template, from, to)) return;
		const next = moveBlock(template, from, to);
		blockKeys.moved(template, next, from, to);
		editor.edit(next);
		editor.select(to);
	}

	/**
	 * The list of a container in a new order (the keys of its rows); the chosen block in it keeps being
	 * chosen. An order of rows the list no longer has (it changed meanwhile) is nothing to apply.
	 */
	function reorderIn(container: Container, order: number[]) {
		const list = blockKeys.listOf(keys, container);
		const from = order.map((k) => list?.indexOf(k) ?? -1);
		if (list === undefined || from.length !== list.length || from.some((i) => i < 0)) return;
		const next = reorder(template, container, from);
		blockKeys.reordered(template, next, container, order);
		editor.edit(next);
		const depth = container.length;
		if (
			selected !== 'page' &&
			selected.length > depth &&
			container.every((n, k) => selected[k] === n)
		) {
			const was = selected[depth]!;
			editor.select(selected.map((n, k) => (k === depth ? from.indexOf(was) : n)));
		}
	}

	/**
	 * «Свой Typst-код»: the block becomes a code block that prints the same, with the code the server
	 * writes for it and its own «Экземпляры»; one step to undo, and the block is the one chosen, its code
	 * shown. A block changed while the code was on its way keeps the change: the code is of what it was.
	 */
	async function toCode(path: BlockPath) {
		const asked = blockAt(template, path);
		if (!asked) return;
		let code: string;
		try {
			({ code } = await blockCode(editor.kind, template, path));
		} catch (e) {
			// The server's reason for this block (`path`/`code_unavailable`), not «Проверьте введённые данные».
			const why = e instanceof ApiError ? e.body?.fields[0]?.message : undefined;
			toast.show({ message: why || errorMessage(e), tone: 'danger' });
			return;
		}
		const now = editor.template;
		const block = blockAt(now, path);
		if (!block || !sameValue(block, asked)) return;
		editor.edit(replaceBlock(now, path, { ...block, body: { type: 'typst_code', code } }));
		editor.select(path);
	}

	function add(type: (typeof BLOCK_TYPES)[number]) {
		const at = placeForNew(template, selected, type);
		editor.edit(insertBlock(template, at, newBlock(type)));
		editor.select(at);
	}

	const addItems = $derived<MenuItem[]>(
		BLOCK_TYPES.map((type) => ({ label: t.blockTypes[type], onselect: () => add(type) }))
	);

	/** «В колонку N» of a top-level block: the end of every column it may go into. */
	function intoColumns(path: BlockPath): MenuItem[] {
		return intoColumnTargets(template, path).map(({ column, block, to }) => {
			const label =
				columnBlocks.length > 1 ? t.toColumnOf(column + 1, block + 1) : t.toColumn(column + 1);
			return { label, icon: ArrowRightToLine, onselect: () => move(path, to) };
		});
	}

	function menuOf(e: Entry): MenuItem[] {
		const out: MenuItem[] = [
			{ label: t.duplicate, icon: Copy, onselect: () => duplicate(e.path) },
			...intoColumns(e.path)
		];
		if (e.path.length === 3) {
			const to = [e.path[0]! + 1];
			out.push({
				label: t.outOfColumns,
				icon: ArrowLeftFromLine,
				onselect: () => move(e.path, to)
			});
		}
		if (convertible(e.block)) {
			out.push({ label: t.toCode, icon: Code, onselect: () => void toCode(e.path) });
		}
		out.push(
			{ separator: true },
			{ label: t.remove, icon: Trash2, danger: true, onselect: () => remove(e.path) }
		);
		return out;
	}
</script>

{#snippet item(e: Entry)}
	{@const label = blockLabel(e.block)}
	<div class="row">
		<button
			type="button"
			title={label}
			class="pick"
			class:on={same(selected, e.path)}
			aria-pressed={same(selected, e.path)}
			onclick={() => editor.select(e.path)}><span class="text">{label}</span></button
		>
		<DropdownMenu items={menuOf(e)} label={t.blockActions(label)}>
			{#snippet trigger(props)}
				<IconButton {...props} size="sm" label={t.blockActions(label)} icon={Ellipsis} />
			{/snippet}
		</DropdownMenu>
	</div>
{/snippet}

{#snippet top(e: Entry)}
	{@render item(e)}
	{#if e.block.body.type === 'columns'}
		{#each e.block.body.columns as column, c (c)}
			<div class="column">
				<span class="column-title">{t.column(c + 1)}</span>
				{#if column.length === 0}
					<p class="empty">{t.emptyColumn}</p>
				{:else}
					<SortableList
						items={columnEntries(e.index, c, column)}
						key={(x) => x.key}
						label={(x) => blockLabel(x.block)}
						row={item}
						onreorder={(order) => reorderIn([e.index, c], order)}
					/>
				{/if}
			</div>
		{/each}
	{/if}
{/snippet}

<div class="blocks">
	<button
		type="button"
		class="pick page"
		class:on={selected === 'page'}
		aria-pressed={selected === 'page'}
		onclick={() => editor.select('page')}
	>
		<FileText size={16} aria-hidden="true" />
		<span class="text">{t.page}</span>
	</button>
	<SortableList
		items={tops}
		key={(e) => e.key}
		label={(e) => blockLabel(e.block)}
		row={top}
		onreorder={(order) => reorderIn([], order)}
	/>
	<div>
		<DropdownMenu items={addItems} label={t.addBlock}>
			{#snippet trigger(props)}
				<Button {...props} icon={Plus}>{t.addBlock}</Button>
			{/snippet}
		</DropdownMenu>
	</div>
</div>

<style>
	.blocks {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
	}
	.pick {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		min-height: var(--control-sm);
		padding: var(--space-1) var(--space-2);
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text);
		font: inherit;
		font-size: var(--text-sm);
		text-align: start;
		cursor: pointer;
	}
	.pick:hover {
		background: var(--surface-2);
	}
	.pick.on {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.pick.page {
		flex: none;
		border-color: var(--border);
		background: var(--surface);
	}
	.pick.page.on {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.text {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.column {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: var(--space-1) 0 0 var(--space-4);
	}
	.column-title {
		color: var(--text-3);
		font-size: var(--text-xs);
		font-weight: 500;
	}
	.empty {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	@media (pointer: coarse) {
		.pick {
			min-height: var(--control-md);
		}
	}
</style>
