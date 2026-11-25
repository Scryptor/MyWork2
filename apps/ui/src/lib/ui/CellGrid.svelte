<!-- The cells of a rack (plan 8.6): shelves from the top down, cells from left to right. A cell is a button
     with its code and what is in it; the occupied one differs from the empty not by colour alone (a solid
     border and the number against a dashed one), the found one has a magnifier. -->
<script lang="ts">
	import Search from '@lucide/svelte/icons/search';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';

	let {
		cells,
		shelves,
		perShelf,
		selected = null,
		onselect,
		label
	}: {
		cells: { shelf: number; cell: number; code: string; positions: number; found: boolean }[];
		shelves: number;
		perShelf: number;
		/** The code of the pressed cell. */
		selected?: string | null;
		onselect?: (code: string) => void;
		/** The name of the group for assistive technology. */
		label: string;
	} = $props();

	const name = (c: { code: string; positions: number }) =>
		`${c.code}, ${c.positions > 0 ? countLabel(c.positions, ru.stock.forms) : ru.stock.map.cellNothing}`;
</script>

<div
	class="grid"
	role="group"
	aria-label={label}
	data-shelves={shelves}
	style:--per-shelf={perShelf}
>
	{#each cells as c (c.code)}
		<button
			type="button"
			class="cell"
			class:occupied={c.positions > 0}
			class:found={c.found}
			aria-label={name(c)}
			aria-pressed={selected === c.code}
			data-found={c.found ? '' : undefined}
			onclick={() => onselect?.(c.code)}
		>
			<span class="code">{c.code}</span>
			<span class="meta">
				{#if c.found}<Search size={14} aria-hidden="true" />{/if}
				{#if c.positions > 0}<span class="num">{c.positions}</span>{/if}
			</span>
		</button>
	{/each}
</div>

<style>
	.grid {
		display: grid;
		grid-template-columns: repeat(var(--per-shelf), minmax(var(--control-lg), 1fr));
		gap: var(--space-1-5);
	}
	.cell {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		gap: var(--space-0-5);
		min-width: 0;
		min-height: var(--space-12);
		padding: var(--space-1) var(--space-1-5);
		border: 1px dashed var(--border);
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text-3);
		font: inherit;
		font-size: var(--text-sm);
		text-align: left;
		cursor: pointer;
	}
	.cell:hover {
		background: var(--surface-2);
	}
	.cell.occupied {
		border: 1px solid var(--border-strong);
		background: var(--surface-2);
		color: var(--text);
	}
	.cell.found {
		border: 2px solid var(--accent);
	}
	.cell[aria-pressed='true'] {
		background: var(--accent-soft);
		color: var(--accent-text);
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
	.code {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.meta {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-1);
		min-height: var(--space-4);
		color: var(--accent-text);
		font-weight: 600;
	}
	.cell.occupied .meta .num {
		color: var(--text);
	}
</style>
