<!-- The rows of a dictionary (and, in Task 7, of statuses and points): a handle to drag, a box to pick for
     a merge, the value that is edited in place, «в N заказах», the archive and delete buttons. The archived
     rows go below under «Архив», dimmed and without a handle. -->
<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { tick, type Snippet } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';
	import IconButton from '#lib/ui/IconButton.svelte';
	import SortableList from '#lib/ui/SortableList.svelte';

	/** `count: null` — not known: no count is shown and nothing is offered for deletion. */
	type Row = { id: number; label: string; archived: boolean; count: number | null };

	let {
		rows,
		countLabel,
		selectable = false,
		selected = $bindable([]),
		readonly,
		onrename,
		onarchive,
		ondelete,
		onreorder,
		extra
	}: {
		rows: Row[];
		countLabel: (n: number) => string;
		selectable?: boolean;
		selected?: number[];
		readonly: boolean;
		onrename: (id: number, label: string) => void;
		onarchive: (id: number, archived: boolean) => void;
		/** Offered only for a value that nothing uses. */
		ondelete?: (id: number) => void;
		onreorder: (ids: number[]) => void;
		/** Something more in a row, after the count (the colour of a status). */
		extra?: Snippet<[number]>;
	} = $props();

	const t = ru.dictionaries;
	const active = $derived(rows.filter((r) => !r.archived));
	const archived = $derived(rows.filter((r) => r.archived));

	let editing = $state<{ id: number; text: string } | null>(null);
	let input = $state<HTMLInputElement | null>(null);

	async function edit(r: Row) {
		if (readonly) return;
		editing = { id: r.id, text: r.label };
		await tick();
		input?.focus();
		input?.select();
	}

	function save(r: Row) {
		const text = editing?.text.trim() ?? '';
		editing = null;
		if (text && text !== r.label) onrename(r.id, text);
	}

	function toggle(id: number, on: boolean) {
		selected = on ? [...selected.filter((x) => x !== id), id] : selected.filter((x) => x !== id);
	}
</script>

{#snippet value(r: Row)}
	{#if editing?.id === r.id}
		<input
			class="edit"
			bind:this={input}
			bind:value={editing.text}
			aria-label={t.valueOf(r.label)}
			onkeydown={(e) => {
				if (e.key === 'Enter') {
					e.preventDefault();
					save(r);
				} else if (e.key === 'Escape') {
					// The edit only: Escape must not also close a layer around the list.
					e.stopPropagation();
					editing = null;
				}
			}}
			onblur={() => (editing = null)}
		/>
	{:else if readonly}
		<span class="text">{r.label}</span>
	{:else}
		<button type="button" class="text editable" onclick={() => void edit(r)}>{r.label}</button>
	{/if}
{/snippet}

{#snippet tail(r: Row)}
	{#if r.count !== null}<span class="count">{countLabel(r.count)}</span>{/if}
	{#if extra}{@render extra(r.id)}{/if}
	{#if !readonly}
		<span class="actions">
			{#if r.archived}
				<IconButton
					size="sm"
					label={t.restore}
					icon={ArchiveRestore}
					onclick={() => onarchive(r.id, false)}
				/>
			{:else}
				<IconButton
					size="sm"
					label={t.toArchive}
					icon={Archive}
					onclick={() => onarchive(r.id, true)}
				/>
			{/if}
			{#if ondelete && r.count === 0}
				<IconButton size="sm" label={t.remove} icon={Trash2} onclick={() => ondelete(r.id)} />
			{/if}
		</span>
	{/if}
{/snippet}

<SortableList
	items={active}
	key={(r) => r.id}
	label={(r) => r.label}
	disabled={readonly}
	{onreorder}
>
	{#snippet row(r)}
		<div class="line">
			{#if selectable && !readonly}
				<input
					type="checkbox"
					class="pick"
					aria-label={t.pick(r.label)}
					checked={selected.includes(r.id)}
					onchange={(e) => toggle(r.id, e.currentTarget.checked)}
				/>
			{/if}
			{@render value(r)}
			{@render tail(r)}
		</div>
	{/snippet}
</SortableList>

{#if archived.length > 0}
	<h3 class="archive">{t.archive}</h3>
	<ul class="archived">
		{#each archived as r (r.id)}
			<li class="line dim">
				{@render value(r)}
				{@render tail(r)}
			</li>
		{/each}
	</ul>
{/if}

<style>
	.line {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--control-md);
		flex-wrap: wrap;
		min-width: 0;
	}
	.pick {
		flex: none;
		margin: 0;
		accent-color: var(--accent);
	}
	.text {
		flex: 1 1 8rem;
		min-width: 0;
		overflow-wrap: anywhere;
		font-size: var(--text-md);
		color: var(--text);
	}
	.editable {
		padding: 0 var(--space-1);
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		font: inherit;
		font-size: var(--text-md);
		text-align: start;
		cursor: text;
	}
	.editable:hover {
		background: var(--surface-3);
	}
	.editable:focus-visible,
	.edit:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 1px;
	}
	.edit {
		flex: 1;
		min-width: 0;
		box-sizing: border-box;
		height: var(--control-sm);
		padding: 0 var(--space-1);
		border: 1px solid var(--accent);
		border-radius: var(--radius-sm);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: var(--text-md);
	}
	.count {
		flex: none;
		font-size: var(--text-sm);
		color: var(--text-3);
		white-space: nowrap;
	}
	.actions {
		display: inline-flex;
		flex: none;
		gap: var(--space-0-5);
	}
	.archive {
		margin: var(--space-3) 0 var(--space-1);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--text-2);
	}
	.archived {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.dim {
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		color: var(--text-3);
		opacity: 0.75;
	}
</style>
