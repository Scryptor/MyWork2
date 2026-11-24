<!-- «Добавить со склада» (decision 4): finds a position, takes a quantity and writes it off into the order.
     The stock is what the server says: when someone took the last units meanwhile, the refusal shows what is
     left and the list is brought up to date. -->
<script lang="ts">
	import Search from '@lucide/svelte/icons/search';
	import { untrack } from 'svelte';
	import type { StockItem } from '$api/StockItem';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { getStockItem } from '#lib/api/stock.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { formatMoney } from '#lib/format/money.js';
	import { ru } from '#lib/i18n/ru.js';
	import type { OrderPartsState } from '#lib/orders/parts.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import QuantityInput from '#lib/ui/QuantityInput.svelte';
	import Spinner from '#lib/ui/Spinner.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { createStockList } from './list.svelte.js';

	let { open = $bindable(false), parts }: { open: boolean; parts: OrderPartsState } = $props();

	const t = ru.orderParts.pick;
	const list = createStockList();
	let text = $state('');
	/** The position chosen: the row of the list as it is now, or as it was when the list lost it. */
	let chosenId = $state<number | null>(null);
	let remembered = $state.raw<StockItem | null>(null);
	let quantity = $state(1);
	let problem = $state<string | null>(null);

	const chosen = $derived(
		chosenId === null ? null : (list.rows.find((r) => r.id === chosenId) ?? remembered)
	);
	const busy = $derived(parts.adding);

	// Every opening starts from an empty search.
	$effect(() => {
		if (!open) return;
		untrack(() => {
			text = '';
			chosenId = null;
			remembered = null;
			quantity = 1;
			problem = null;
		});
	});

	// The list follows the search a moment after the last key; the first read is at once. Until the answer
	// comes the rows are the old ones: they cannot be chosen, so a click does not land on a row that moves.
	let loadedFor: string | null = null;
	let waiting = $state(false);
	$effect(() => {
		if (!open) {
			loadedFor = null;
			waiting = false;
			return;
		}
		const query = text.trim();
		waiting = true;
		const timer = setTimeout(
			() => {
				loadedFor = query;
				waiting = false;
				list.load({ query: query || undefined, sort: 'name' });
			},
			loadedFor === null ? 0 : 250
		);
		return () => clearTimeout(timer);
	});
	const refreshing = $derived(waiting || list.loading);

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});

	function choose(item: StockItem) {
		chosenId = item.id;
		remembered = item;
		quantity = Math.min(Math.max(quantity, 1), item.quantity);
		problem = null;
	}

	/** The position as the server has it now; the list and the choice take it over. */
	async function refresh(id: number): Promise<StockItem | null> {
		try {
			const fresh = await getStockItem(id);
			list.update(id, () => fresh);
			remembered = fresh;
			return fresh;
		} catch (e) {
			if (e instanceof ApiError && e.status === 404) return null;
			throw e;
		}
	}

	async function submit() {
		const item = chosen;
		if (busy || !item || item.quantity === 0) return;
		problem = null;
		try {
			await parts.add(item.id, quantity);
			open = false;
		} catch (e) {
			await explain(e, item);
		}
	}

	/** The position is gone: the list is read again without it. */
	function gone() {
		problem = t.deleted;
		chosenId = null;
		list.load({ query: text.trim() || undefined, sort: 'name' });
	}

	async function explain(e: unknown, item: StockItem) {
		const fields = e instanceof ApiError && e.code === 'validation' ? (e.body?.fields ?? []) : [];
		const refused = (field: string, code: string) =>
			fields.some((f) => f.field === field && f.code === code);
		if (refused('stock_item_id', 'deleted')) return gone();
		if (refused('quantity', 'not_enough_stock')) {
			try {
				const fresh = await refresh(item.id);
				if (fresh === null) return gone();
				quantity = Math.min(Math.max(quantity, 1), Math.max(fresh.quantity, 1));
				problem = t.notEnough(fresh.quantity);
			} catch (again) {
				problem = t.failed(errorMessage(again));
			}
			return;
		}
		problem = t.failed(errorMessage(e));
	}
</script>

<Dialog bind:open title={t.title} dismissible={!busy}>
	<div class="pick">
		<TextInput
			type="search"
			ariaLabel={t.search}
			placeholder={t.searchPlaceholder}
			autocomplete="off"
			bind:value={text}
		>
			{#snippet prefix()}<Search size={16} aria-hidden="true" />{/snippet}
		</TextInput>

		{#if list.error && list.rows.length === 0}
			<p class="error" role="alert">{t.loadFailed}</p>
			<div>
				<Button
					size="sm"
					onclick={() => list.load({ query: text.trim() || undefined, sort: 'name' })}
					>{ru.common.retry}</Button
				>
			</div>
		{:else if list.rows.length === 0}
			{#if list.loading}
				<div class="wait"><Spinner /></div>
			{:else}
				<p class="muted">{t.none}. {t.noneHint}</p>
			{/if}
		{:else}
			<ul class="rows" aria-label={t.list} aria-busy={refreshing}>
				{#each list.rows as item (item.id)}
					<li>
						<button
							type="button"
							class="row"
							aria-pressed={chosenId === item.id}
							disabled={item.quantity === 0 || busy}
							onclick={() => choose(item)}
						>
							<span class="name">{item.display_name}</span>
							<span class="num">{ru.stock.pieces(item.quantity)}</span>
							<span class="num price">{formatMoney(item.price)}</span>
							{#if item.quantity === 0}<span class="muted">{t.outOfStock}</span>{/if}
						</button>
					</li>
				{/each}
			</ul>
			{#if list.rows.length < list.total}
				<div>
					<Button size="sm" busy={list.loading} onclick={() => list.loadMore()}>{t.more}</Button>
				</div>
			{/if}
		{/if}

		{#if chosen && chosen.quantity > 0}
			<QuantityInput label={t.quantity} min={1} max={chosen.quantity} bind:value={quantity} />
		{/if}
		{#if problem}<p class="error" role="alert">{problem}</p>{/if}
	</div>
	{#snippet footer()}
		<Button disabled={busy} onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button
			variant="primary"
			{busy}
			disabled={!chosen || chosen.quantity === 0}
			onclick={() => void submit()}>{t.submit}</Button
		>
	{/snippet}
</Dialog>

<style>
	.pick {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.rows {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		max-height: 40vh;
		margin: 0;
		padding: 0;
		overflow-y: auto;
		list-style: none;
	}
	.rows[aria-busy='true'] {
		opacity: 0.6;
		pointer-events: none;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-3);
		width: 100%;
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		text-align: start;
		cursor: pointer;
	}
	.row:hover:not(:disabled) {
		background: var(--surface-2);
	}
	.row[aria-pressed='true'] {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.row:focus-visible {
		outline: 2px solid var(--focus);
	}
	.row:disabled {
		color: var(--text-disabled);
		cursor: not-allowed;
	}
	.name {
		flex: 1 1 12rem;
		min-width: 0;
		overflow-wrap: anywhere;
		font-weight: 500;
	}
	.price {
		margin-inline-start: auto;
	}
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.error {
		margin: 0;
		color: var(--danger-text);
	}
	.wait {
		display: flex;
		justify-content: center;
		padding: var(--space-4);
	}
</style>
