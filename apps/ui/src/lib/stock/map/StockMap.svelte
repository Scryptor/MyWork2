<!-- «Карта» склада (plan 8.6): the racks as cards with the grids of their cells, the cell found by the
     filter marked. The point, the open cell and the filter live in the address of «Склад»; the arranging
     mode («Изменить расстановку») is only of this screen. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import Warehouse from '@lucide/svelte/icons/warehouse';
	import { untrack } from 'svelte';
	import type { StockCellRef } from '$api/StockCellRef';
	import type { StockRack } from '$api/StockRack';
	import type { StockRackQuery } from '$api/StockRackQuery';
	import { errorMessage } from '#lib/api/client.js';
	import { deleteStockRack, listStockRacks, reorderStockRacks } from '#lib/api/stock.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { openPreview } from '#lib/print/print.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import CellPanel from './CellPanel.svelte';
	import RackCard from './RackCard.svelte';
	import RackDialog from './RackDialog.svelte';
	import { cellParam, moveRack, NO_POINT, pointOptions, racksOfPoint } from './racks.js';

	let {
		query,
		point,
		cell,
		canEdit,
		onpoint,
		oncell,
		onopenitem
	}: {
		/** The filter of the list: the cells with a matching position are marked. */
		query: StockRackQuery;
		/** The point chosen in the address; none — the point of this computer, or the first. */
		point: number | typeof NO_POINT | null;
		/** The cell open in the panel (from the address). */
		cell: StockCellRef | null;
		canEdit: boolean;
		onpoint: (value: number | typeof NO_POINT) => void;
		oncell: (code: string | null) => void;
		onopenitem: (id: number) => void;
	} = $props();

	const t = ru.stock.map;

	let racks = $state.raw<StockRack[]>([]);
	let loaded = $state(false);
	let failed = $state<string | null>(null);
	let editing = $state(false);
	let rackDialog = $state<{ open: boolean; rack?: StockRack }>({ open: false });
	let deleting = $state<StockRack | null>(null);
	let confirming = $state(false);

	let ask = 0;
	/** The racks as the server has them now; a slower older answer never replaces a newer one. */
	async function load() {
		const mine = ++ask;
		try {
			const next = await listStockRacks(query);
			if (mine !== ask) return;
			racks = next;
			failed = null;
		} catch (e) {
			if (mine !== ask) return;
			failed = errorMessage(e);
		} finally {
			if (mine === ask) loaded = true;
		}
	}
	const queryKey = $derived(JSON.stringify(query));
	$effect(() => {
		void queryKey;
		untrack(() => void load());
	});

	const options = $derived(pointOptions(racks, app.points));
	const chosen = $derived.by(() => {
		const wanted = [point, app.point].map((p) => (p === null ? null : String(p)));
		return wanted.find((w) => options.some((o) => o.value === w)) ?? options[0]?.value ?? null;
	});
	const shown = $derived(
		options.length > 1 && chosen !== null ? racksOfPoint(racks, chosen) : racks
	);
	/** The point is offered in `SegmentedControl` as text; the address keeps a number or `none`. */
	const pointValue = $derived(chosen ?? '');

	const target = $derived.by(() => {
		if (!cell) return null;
		const rack = racks.find((r) => r.id === cell.rack_id);
		const found = rack?.cells.find((c) => c.shelf === cell.shelf && c.cell === cell.cell);
		return found ? { ref: cell, code: found.code } : null;
	});

	async function move(rack: StockRack, dir: -1 | 1) {
		const order = moveRack(racks, rack.id, dir);
		if (!order) return;
		try {
			await reorderStockRacks(
				order,
				racks.map((r) => r.id)
			);
		} catch (e) {
			toast.show({ message: t.reorderFailed(errorMessage(e)), tone: 'danger' });
		}
		await load();
	}

	async function remove() {
		const rack = deleting;
		if (!rack) return;
		try {
			await deleteStockRack(rack.id);
			toast.show({ message: t.deleted(rack.code), tone: 'success' });
		} catch (e) {
			toast.show({ message: t.deleteFailed(errorMessage(e)), tone: 'danger' });
		}
		await load();
	}

	const stickers = (rack: StockRack) =>
		openPreview('cell_sticker', { rackId: rack.id, code: rack.code });
	const addRack = () => (rackDialog = { open: true });
</script>

<div class="map">
	{#if canEdit && racks.length > 0}
		<div class="bar">
			<Switch label={t.editMode} bind:checked={editing} />
			{#if editing}
				<Button size="sm" icon={Plus} onclick={addRack}>{t.addRack}</Button>
			{/if}
		</div>
	{/if}
	{#if options.length > 1}
		<SegmentedControl
			label={t.points}
			{options}
			bind:value={() => pointValue, (v) => onpoint(v === NO_POINT ? NO_POINT : Number(v))}
		/>
	{/if}

	{#if !loaded}
		<Skeleton />
	{:else if failed !== null}
		<EmptyState title={t.loadFailed} description={failed}>
			{#snippet actions()}
				<Button size="sm" onclick={() => void load()}>{ru.common.retry}</Button>
			{/snippet}
		</EmptyState>
	{:else if racks.length === 0}
		<EmptyState icon={Warehouse} title={t.empty} description={canEdit ? t.emptyHint : undefined}>
			{#snippet actions()}
				{#if canEdit}
					<Button variant="primary" icon={Plus} onclick={addRack}>{t.addRack}</Button>
				{/if}
			{/snippet}
		</EmptyState>
	{:else}
		<div class="racks">
			{#each shown as rack (rack.id)}
				<RackCard
					{rack}
					selected={target !== null && target.ref.rack_id === rack.id ? target.code : null}
					{editing}
					canMoveUp={moveRack(racks, rack.id, -1) !== null}
					canMoveDown={moveRack(racks, rack.id, 1) !== null}
					onselect={(code) => {
						const c = rack.cells.find((x) => x.code === code);
						if (c) oncell(cellParam(rack.id, c.shelf, c.cell));
					}}
					onedit={() => (rackDialog = { open: true, rack })}
					ondelete={() => {
						deleting = rack;
						confirming = true;
					}}
					onmove={(dir) => void move(rack, dir)}
					onstickers={() => stickers(rack)}
				/>
			{/each}
		</div>
	{/if}
</div>

<CellPanel
	{target}
	{racks}
	{canEdit}
	onclose={() => oncell(null)}
	onchanged={() => void load()}
	{onopenitem}
/>

{#if canEdit}
	<RackDialog
		bind:open={rackDialog.open}
		rack={rackDialog.rack}
		points={app.activePoints}
		onsaved={() => void load()}
	/>
	<ConfirmDialog
		bind:open={confirming}
		title={t.deleteTitle(deleting?.code ?? '')}
		description={t.deleteText}
		confirmLabel={t.deleteRack}
		onconfirm={remove}
	/>
{/if}

<style>
	.map {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		flex: 1;
		min-height: 0;
		overflow: auto;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: var(--space-4);
	}
	.racks {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
		align-items: start;
		gap: var(--space-3);
	}
</style>
