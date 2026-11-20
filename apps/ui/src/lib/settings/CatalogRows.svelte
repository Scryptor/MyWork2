<!-- The rows of statuses, points and dictionaries: one grid with the same columns for every row — the handle
     to drag, a box to pick for a merge, the name edited in place, the count, the columns of the screen (the
     colour and the kind of a status), the buttons. Over them a row of the column titles, and a row of the same
     columns for the form of a new one (`add`). The archived rows go below under «Архив», dimmed and without a
     handle. A narrow window puts a row with columns of the screen on two lines; the columns still line up. -->
<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
	import SpellCheck from '@lucide/svelte/icons/spell-check';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { tick, type Snippet } from 'svelte';
	import { display } from '#lib/design/prefs.svelte.js';
	import { groupDigits } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import SortableList from '#lib/ui/SortableList.svelte';
	import VisuallyHidden from '#lib/ui/VisuallyHidden.svelte';

	/** `count: null` — not known: no count is shown and nothing is offered for deletion. */
	type Row = { id: number; label: string; archived: boolean; count: number | null };
	/** A column of the screen after the count. Its cells are placed by `style:grid-area={id}`. */
	type Column = { id: string; title: string; help?: string | readonly string[]; width: string };

	let {
		rows,
		countLabel,
		nameTitle = ru.catalogRows.name,
		countTitle = ru.catalogRows.orders,
		columns = [],
		selectable = false,
		selected = $bindable([]),
		readonly,
		onrename,
		onarchive,
		ondelete,
		onalign,
		onreorder,
		extra,
		add
	}: {
		rows: Row[];
		/** The count in words, for a screen reader: «в 3 заказах». */
		countLabel: (n: number) => string;
		nameTitle?: string;
		/** The title of the count: what is counted. */
		countTitle?: string;
		columns?: Column[];
		selectable?: boolean;
		selected?: number[];
		readonly: boolean;
		onrename: (id: number, label: string) => void;
		onarchive: (id: number, archived: boolean) => void;
		/** Offered only for a value that nothing uses. */
		ondelete?: (id: number) => void;
		/** Offered for a value that records use: their spelling is made the one of the dictionary. */
		onalign?: (id: number) => void;
		onreorder: (ids: number[]) => void;
		/**
		 * The cells of `columns` in a row, each with `style:grid-area` of its column. In two lines (a narrow
		 * window) the last column takes the room that is left: its cells keep to their own width.
		 */
		extra?: Snippet<[number]>;
		/** The form of a new one, in the same columns: its cells carry `style:grid-area` (`name`, a column, `actions`). */
		add?: Snippet;
	} = $props();

	const t = ru.dictionaries;
	const active = $derived(rows.filter((r) => !r.archived));
	const archived = $derived(rows.filter((r) => r.archived));

	const pick = $derived(selectable && !readonly);
	const hasActions = $derived(!readonly);
	/** On a phone «В архив» is an icon (its words are its name and its tooltip). */
	const iconOnly = $derived(display.phone);
	const twoLines = $derived(columns.length > 0 && display.narrow);

	/**
	 * The columns of every row: constant widths but the name's, so one row's are another's. The handle is
	 * outside the grid, in front of it (a space as wide in the rows that have none).
	 */
	const grid = $derived.by(() => {
		const archive = iconOnly ? 'var(--cat-icon)' : 'var(--cat-archive)';
		// A dictionary has a second button in some rows: «Привести написание» or «Удалить».
		const actions =
			onalign || ondelete ? `calc(${archive} + var(--space-0-5) + var(--cat-icon))` : archive;
		const ids = columns.map((c) => c.id);
		const widths = columns.map((c) => c.width);
		const p = pick ? ['pick'] : [];
		const pw = pick ? ['var(--cat-pick)'] : [];
		const line = (cells: string[]) => `"${cells.join(' ')}"`;
		if (!twoLines) {
			const a = hasActions ? ['actions'] : [];
			return {
				columns: [
					...pw,
					'minmax(0, 1fr)',
					'var(--cat-count)',
					...widths,
					...(hasActions ? [actions] : [])
				].join(' '),
				areas: line([...p, 'name', 'count', ...ids, ...a]),
				// The form has no count: its name takes that room too.
				add: line([...p, 'name', 'name', ...ids, ...a])
			};
		}
		// The name and the count on the first line, the columns and the buttons under them on the second. The
		// last column takes the room that is left (its cells keep to their width themselves): a phone has
		// little of it.
		const right = hasActions ? `max(var(--cat-count), ${actions})` : 'var(--cat-count)';
		const under = pick ? ['.'] : [];
		return {
			columns: [...pw, ...widths.slice(0, -1), 'minmax(0, 1fr)', right].join(' '),
			areas: [
				line([...p, ...ids.map(() => 'name'), 'count']),
				line([...under, ...ids, hasActions ? 'actions' : '.'])
			].join(' '),
			// The form: the name across, its button under the count.
			add: [
				line([...p, ...ids.map(() => 'name'), 'name']),
				line([...under, ...ids, 'actions'])
			].join(' ')
		};
	});

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

{#snippet archiveButton(r: Row)}
	{@const label = r.archived ? t.restore : t.toArchive}
	{@const icon = r.archived ? ArchiveRestore : Archive}
	{@const onclick = () => onarchive(r.id, !r.archived)}
	{#if iconOnly}
		<IconButton size="sm" {label} {icon} {onclick} />
	{:else}
		<Button size="sm" variant="ghost" {icon} {onclick}>{label}</Button>
	{/if}
{/snippet}

<!-- The cells of a row in the grid. -->
{#snippet cells(r: Row)}
	{#if pick && !r.archived}
		<input
			type="checkbox"
			class="pick"
			aria-label={t.pick(r.label)}
			checked={selected.includes(r.id)}
			onchange={(e) => toggle(r.id, e.currentTarget.checked)}
		/>
	{/if}
	<div class="name">{@render value(r)}</div>
	<span class="count">
		{#if r.count !== null}
			<span aria-hidden="true">{groupDigits(String(r.count))}</span>
			<VisuallyHidden>{countLabel(r.count)}</VisuallyHidden>
		{/if}
	</span>
	{#if extra}{@render extra(r.id)}{/if}
	{#if hasActions}
		<span class="actions">
			{@render archiveButton(r)}
			{#if onalign && r.count !== null && r.count > 0}
				<IconButton size="sm" label={t.align} icon={SpellCheck} onclick={() => onalign(r.id)} />
			{/if}
			{#if ondelete && r.count === 0}
				<IconButton size="sm" label={t.remove} icon={Trash2} onclick={() => ondelete(r.id)} />
			{/if}
		</span>
	{/if}
{/snippet}

<!-- In front of the grid of a row without a handle: as wide as the handle of the rows that have one. -->
{#snippet handleSpace()}
	{#if !readonly}<span class="handle-space" aria-hidden="true"></span>{/if}
{/snippet}

<div
	class="catalog"
	class:two-lines={twoLines}
	class:phone={iconOnly}
	style:--cat-columns={grid.columns}
	style:--cat-areas={grid.areas}
	style:--cat-add-areas={grid.add}
>
	{#if rows.length > 0 || add}
		<div class="row catalog-head">
			{@render handleSpace()}
			<div class="grid" data-testid="catalog-head">
				<span class="title" style:grid-area="name">{nameTitle}</span>
				<span class="title count-title" style:grid-area="count">{countTitle}</span>
				{#each columns as c (c.id)}
					<span class="title" style:grid-area={c.id}>
						{c.title}
						{#if c.help}<HelpHint title={c.title} text={c.help} />{/if}
					</span>
				{/each}
			</div>
		</div>
	{/if}

	{#if add}
		<div class="row catalog-add">
			{@render handleSpace()}
			<div class="grid">{@render add()}</div>
		</div>
	{/if}

	{#if active.length > 0}
		<SortableList
			items={active}
			key={(r) => r.id}
			label={(r) => r.label}
			disabled={readonly}
			{onreorder}
		>
			{#snippet row(r)}
				<div class="grid">{@render cells(r)}</div>
			{/snippet}
		</SortableList>
	{/if}

	{#if archived.length > 0}
		<h3 class="archive">{t.archive}</h3>
		<ul class="archived">
			{#each archived as r (r.id)}
				<li class="dim">
					{@render handleSpace()}
					<div class="grid">{@render cells(r)}</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.catalog {
		/* The constant widths of the grid (they grow with the scale: rem). */
		--cat-pick: 1rem;
		--cat-count: 3.5rem;
		--cat-archive: 6rem;
		--cat-icon: var(--control-sm);
		--cat-handle: var(--control-sm);
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}
	@media (pointer: coarse) {
		.catalog {
			--cat-icon: var(--control-md);
			--cat-handle: var(--control-md);
		}
	}
	/* Two lines on a phone: the count is as narrow as its title, so the list of the kind keeps its words. */
	.two-lines.phone {
		--cat-count: 3rem;
	}
	/* The rows that are not items of the list take the item's padding and border, so the grids line up. */
	.row,
	.dim {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-2);
		border: 1px solid transparent;
		border-radius: var(--radius-md);
	}
	.catalog-head {
		padding-block: 0;
	}
	.handle-space {
		flex: none;
		width: var(--cat-handle);
	}
	.grid {
		flex: 1;
		display: grid;
		grid-template-columns: var(--cat-columns);
		grid-template-areas: var(--cat-areas);
		align-items: center;
		column-gap: var(--space-2);
		row-gap: var(--space-1);
		min-width: 0;
		min-height: var(--control-md);
	}
	.catalog-head .grid {
		min-height: 0;
	}
	/* A mistake under the name of the form makes it taller: the other cells stay at the top. */
	.catalog-add .grid {
		grid-template-areas: var(--cat-add-areas);
		align-items: start;
	}
	.title {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--text-3);
	}
	.count-title {
		justify-self: end;
	}
	.pick {
		grid-area: pick;
		justify-self: start;
		width: var(--cat-pick);
		height: var(--cat-pick);
		margin: 0;
		accent-color: var(--accent);
	}
	.name {
		grid-area: name;
		display: flex;
		min-width: 0;
	}
	.text {
		min-width: 0;
		overflow-wrap: anywhere;
		font-size: var(--text-md);
		color: var(--text);
	}
	.editable {
		padding: 0 var(--space-1);
		margin-inline-start: calc(-1 * var(--space-1));
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
		grid-area: count;
		justify-self: end;
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
		color: var(--text-3);
		white-space: nowrap;
	}
	/* «В архив» first; the second button of a dictionary at the end, in its own place whatever the words of
	   the first. */
	.actions {
		grid-area: actions;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-0-5);
		min-width: 0;
	}
	.two-lines .actions {
		justify-self: end;
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
		border-color: var(--border);
		color: var(--text-3);
		opacity: 0.75;
	}
</style>
