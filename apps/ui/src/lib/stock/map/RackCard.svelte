<!-- A rack of the map (plan 8.6): its title and the grid of its cells; in the arranging mode also the
     actions on the rack itself. -->
<script lang="ts">
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Printer from '@lucide/svelte/icons/printer';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import type { StockRack } from '$api/StockRack';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import CellGrid from '#lib/ui/CellGrid.svelte';

	let {
		rack,
		selected = null,
		editing = false,
		canMoveUp = false,
		canMoveDown = false,
		onselect,
		onedit,
		ondelete,
		onmove,
		onstickers
	}: {
		rack: StockRack;
		/** The code of the cell open in the panel. */
		selected?: string | null;
		/** The arranging mode: the actions on the rack are shown. */
		editing?: boolean;
		canMoveUp?: boolean;
		canMoveDown?: boolean;
		onselect: (code: string) => void;
		onedit: () => void;
		ondelete: () => void;
		onmove: (dir: -1 | 1) => void;
		onstickers: () => void;
	} = $props();

	const t = ru.stock.map;
	const named = (what: string) => t.action(what, rack.code);
</script>

<Card padding="sm">
	<div class="rack">
		<h3 class="title">{t.rackTitle(rack.code, rack.label)}</h3>
		<CellGrid
			cells={rack.cells}
			shelves={rack.shelves}
			perShelf={rack.cells_per_shelf}
			{selected}
			{onselect}
			label={t.rackGroup(rack.code)}
		/>
		{#if editing}
			<div class="actions">
				<Button size="sm" icon={Pencil} aria-label={named(t.editRack)} onclick={onedit}
					>{t.editRack}</Button
				>
				<Button
					size="sm"
					icon={ArrowUp}
					disabled={!canMoveUp}
					aria-label={named(t.up)}
					onclick={() => onmove(-1)}>{t.up}</Button
				>
				<Button
					size="sm"
					icon={ArrowDown}
					disabled={!canMoveDown}
					aria-label={named(t.down)}
					onclick={() => onmove(1)}>{t.down}</Button
				>
				<Button size="sm" icon={Printer} aria-label={named(t.stickers)} onclick={onstickers}
					>{t.stickers}</Button
				>
				<Button
					size="sm"
					variant="danger"
					icon={Trash2}
					aria-label={named(t.deleteRack)}
					onclick={ondelete}>{t.deleteRack}</Button
				>
			</div>
		{/if}
	</div>
</Card>

<style>
	.rack {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.title {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
