<!-- The panel of a cell of the map (plan 8.6): what lies in it, «Положить сюда…» and the sticker of the cell.
     Which cell is open lives in the address of «Склад» (`cell=`); the panel only reads and tells. -->
<script lang="ts">
	import Printer from '@lucide/svelte/icons/printer';
	import { untrack } from 'svelte';
	import type { StockCellRef } from '$api/StockCellRef';
	import type { StockItem } from '$api/StockItem';
	import type { StockRack } from '$api/StockRack';
	import { getStockItem, listStock, putStock } from '#lib/api/stock.js';
	import { ru } from '#lib/i18n/ru.js';
	import { openPreview } from '#lib/print/print.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import SlideOver from '#lib/ui/SlideOver.svelte';
	import PlaceDialog from '../PlaceDialog.svelte';

	let {
		target,
		racks,
		canEdit,
		onclose,
		onchanged,
		onopenitem
	}: {
		/** The cell open (from the address); none — the panel is closed. */
		target: { ref: StockCellRef; code: string } | null;
		racks: StockRack[];
		canEdit: boolean;
		onclose: () => void;
		/** Something was laid into the cell: the map counts again. */
		onchanged: () => void;
		/** A position of the cell was chosen: it opens in «Список». */
		onopenitem: (id: number) => void;
	} = $props();

	const t = ru.stock.map;

	let items = $state.raw<StockItem[] | null>(null);
	let failed = $state(false);
	let putting = $state(false);

	/** The units of the position lying in this cell. */
	const quantityHere = (i: StockItem) =>
		i.places.find((p) => p.code === target?.code)?.quantity ?? 0;

	async function load(code: string, signal: { current: boolean }) {
		try {
			const page = await listStock({ place: code, sort: 'name', limit: 200 });
			if (signal.current) items = page.items;
		} catch {
			if (signal.current) failed = true;
		}
	}
	$effect(() => {
		const code = target?.code;
		items = null;
		failed = false;
		if (code === undefined) return;
		const signal = { current: true };
		untrack(() => void load(code, signal));
		return () => (signal.current = false);
	});

	async function put(id: number, ref: StockCellRef, quantity: number) {
		await putStock(id, { place: ref, quantity });
		if (target) await load(target.code, { current: true });
		onchanged();
	}

	function print() {
		if (!target) return;
		const { rack_id, shelf, cell } = target.ref;
		openPreview('cell_sticker', { rackId: rack_id, shelf, cell, code: target.code });
	}
</script>

<SlideOver
	bind:open={() => target !== null, (v) => !v && onclose()}
	title={t.cell(target?.code ?? '')}
>
	<div class="body">
		<div class="actions">
			{#if canEdit}
				<Button variant="primary" onclick={() => (putting = true)}>{t.putHere}</Button>
			{/if}
			<Button icon={Printer} onclick={print}>{t.printSticker}</Button>
		</div>
		<Section title={t.cellItems}>
			{#if failed}
				<Callout tone="danger">{t.cellLoadFailed}</Callout>
			{:else if items === null}
				<Skeleton />
			{:else if items.length === 0}
				<p class="hint">{t.cellEmpty}</p>
			{:else}
				<ul>
					{#each items as i (i.id)}
						<li>
							<button type="button" class="item" onclick={() => onopenitem(i.id)}>
								<span class="name">{i.display_name}</span>
								<span class="num">{ru.stock.pieces(quantityHere(i))}</span>
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</Section>
	</div>
</SlideOver>

{#if canEdit && target}
	<PlaceDialog
		bind:open={putting}
		mode="here"
		{racks}
		cell={target}
		onsubmit={put}
		reload={getStockItem}
	/>
{/if}

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-4);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	ul {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.item {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		width: 100%;
		padding: var(--space-2);
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.item:hover {
		background: var(--surface-2);
	}
	.name {
		font-weight: 500;
	}
	.hint {
		margin: 0;
		color: var(--text-3);
	}
</style>
