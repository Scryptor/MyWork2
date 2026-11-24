<!-- The panel of a stock position (decision 18). Here it only shows the position, read-only: editing,
     photos and history come with the next task. Which position is open lives in the address of «Склад». -->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { StockItem } from '$api/StockItem';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { getStockItem } from '#lib/api/stock.js';
	import { formatMoney } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import SlideOver from '#lib/ui/SlideOver.svelte';
	import { STOCK_FIELDS } from './fields.js';

	let { itemId, onclose }: { itemId: number | null; onclose: () => void } = $props();

	const t = ru.stock;
	let item = $state.raw<StockItem | null>(null);
	let loadError = $state<string | null>(null);
	/** The position is not there: deleted. Asking again cannot help. */
	let gone = $state(false);
	let token = 0;

	$effect(() => {
		const id = itemId;
		untrack(() => void load(id));
	});

	async function load(id: number | null) {
		const mine = ++token;
		loadError = null;
		gone = false;
		if (id === null) {
			item = null;
			return;
		}
		if (item?.id === id) return;
		// The position before is not shown while the next one loads.
		item = null;
		try {
			const loaded = await getStockItem(id);
			if (mine === token) item = loaded;
		} catch (e) {
			if (mine !== token) return;
			gone = e instanceof ApiError && e.code === 'not_found';
			loadError = errorMessage(e);
		}
	}

	const title = $derived(item ? item.display_name : t.number(itemId ?? 0));
	const dash = (v: string) => (v === '' ? '—' : v);
	const rows = $derived(
		item
			? [
					{ label: STOCK_FIELDS.name, value: dash(item.name) },
					{ label: STOCK_FIELDS.part_type, value: dash(item.part_type) },
					{ label: STOCK_FIELDS.device_type, value: dash(item.device_type) },
					{ label: STOCK_FIELDS.brand, value: dash(item.brand) },
					{ label: STOCK_FIELDS.model, value: dash(item.model) },
					{ label: STOCK_FIELDS.color, value: dash(item.color) },
					{ label: STOCK_FIELDS.quantity, value: String(item.quantity) },
					{ label: STOCK_FIELDS.price, value: formatMoney(item.price) },
					{
						label: STOCK_FIELDS.restock_threshold,
						value: item.restock_threshold > 0 ? String(item.restock_threshold) : '—'
					},
					{ label: STOCK_FIELDS.note, value: dash(item.note) }
				]
			: []
	);
</script>

<SlideOver bind:open={() => itemId !== null, (v) => !v && onclose()} {title}>
	{#snippet header()}
		{#if item}<p class="meta num">{t.number(item.id)}</p>{/if}
	{/snippet}

	{#if item}
		<KeyValue items={rows} />
	{:else if gone}
		<Callout tone="info" title={t.gone}>{t.goneHint}</Callout>
	{:else if loadError}
		<Callout tone="danger" title={t.loadItemFailed}>
			{loadError}
			{#snippet actions()}
				<Button size="sm" onclick={() => void load(itemId)}>{ru.common.retry}</Button>
			{/snippet}
		</Callout>
	{:else}
		<div class="loading" aria-busy="true">
			<Skeleton height="2.5rem" />
			<Skeleton height="4rem" />
			<Skeleton height="2.5rem" width="80%" />
		</div>
	{/if}
</SlideOver>

<style>
	.meta {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.loading {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
</style>
