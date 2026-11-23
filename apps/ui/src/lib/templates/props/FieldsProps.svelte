<!-- The properties of a «Поля» block: its rows, each with its title, value, «Скрывать, если пусто» and
     «Жирная»; «Добавить строку» and «Удалить строку». The rows are dragged to a new order. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import type { BlockBody } from '$api/BlockBody';
	import type { FieldRow } from '$api/FieldRow';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import SortableList from '#lib/ui/SortableList.svelte';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';
	import FieldRowProps from './FieldRowProps.svelte';

	type Body = Extract<BlockBody, { type: 'fields' }>;
	/** What the template allows (`Template::validate`). */
	const MAX_ROWS = 50;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'fields' ? block.body : null);
	const key = $derived(path.join('.'));

	// The rows keep their keys while they are added, removed and moved here; a list changed otherwise
	// (an undo) gets new ones.
	let ids: number[] = [];
	let nextId = 0;
	const rows = $derived.by(() => {
		const list = body?.rows ?? [];
		if (ids.length !== list.length) ids = list.map(() => nextId++);
		return list.map((row, i) => ({ row, i, id: ids[i]! }));
	});

	function setRows(next: FieldRow[], field?: string) {
		if (!block || !body) return;
		editor.edit(
			replaceBlock(editor.template, path, { ...block, body: { ...body, rows: next } as Body }),
			field === undefined ? undefined : `${key}.${field}`
		);
	}

	function change(i: number, patch: Partial<FieldRow>, field?: keyof FieldRow) {
		if (!body) return;
		setRows(
			body.rows.map((r, j) => (j === i ? { ...r, ...patch } : r)),
			field === undefined ? undefined : `${i}.${field}`
		);
	}

	function add() {
		if (!body) return;
		ids = [...ids, nextId++];
		setRows([...body.rows, { label: '', value: '', hide_if_empty: false, bold: false }]);
	}

	function remove(i: number) {
		if (!body) return;
		ids = ids.filter((_, j) => j !== i);
		setRows(body.rows.filter((_, j) => j !== i));
	}

	function reorder(order: number[]) {
		if (!body) return;
		const from = order.map((id) => ids.indexOf(id));
		if (from.length !== body.rows.length || from.some((i) => i < 0)) return;
		ids = order;
		setRows(from.map((i) => body.rows[i]!));
	}
</script>

{#if body}
	<div class="props">
		<SortableList
			items={rows}
			key={(r) => r.id}
			label={(r) => r.row.label || t.fieldRow(r.i + 1)}
			row={item}
			onreorder={reorder}
		/>
		<div>
			<Button icon={Plus} disabled={body.rows.length >= MAX_ROWS} onclick={add}>{t.addRow}</Button>
		</div>
	</div>
{/if}

{#snippet item(r: { row: FieldRow; i: number })}
	<FieldRowProps
		row={r.row}
		number={r.i + 1}
		onchange={(patch, field) => change(r.i, patch, field)}
		onremove={() => remove(r.i)}
	/>
{/snippet}

<style>
	.props {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
</style>
