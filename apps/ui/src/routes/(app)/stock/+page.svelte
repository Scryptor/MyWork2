<!-- «Склад» (decision 18): the list filter, «Все» / «Докупить», the table (cards on a phone) and the panel
     of a position. The view and the position open live in the address: view.ts reads and writes them. -->
<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import Plus from '@lucide/svelte/icons/plus';
	import Search from '@lucide/svelte/icons/search';
	import Warehouse from '@lucide/svelte/icons/warehouse';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { StockItem } from '$api/StockItem';
	import { app } from '#lib/app/context.svelte.js';
	import { createListFilter } from '#lib/app/list-filter.svelte.js';
	import { countLabel, formatMoney } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import { stockColumns } from '#lib/stock/columns.js';
	import { createStockList } from '#lib/stock/list.svelte.js';
	import StockAddDialog from '#lib/stock/StockAddDialog.svelte';
	import StockItemPanel from '#lib/stock/StockItemPanel.svelte';
	import {
		defaultStockView,
		openStockItemId,
		shownStockChips,
		stockSearch,
		stockViewFromSearch,
		stockViewToQuery,
		withStockChips,
		type StockView
	} from '#lib/stock/view.js';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import FilterChips from '#lib/ui/FilterChips.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import InfiniteList from '#lib/ui/InfiniteList.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';
	import ListCard from '#lib/ui/ListCard.svelte';
	import DataTable from '#lib/ui/table/DataTable.svelte';
	import type { RowId } from '#lib/ui/table/types.js';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.stock;

	const canView = $derived(app.can('stock_view'));
	const canCreate = $derived(app.can('stock_create'));
	const view = $derived(stockViewFromSearch(page.url.searchParams));
	const openId = $derived(openStockItemId(page.url.searchParams));

	// The list filter: its text is ahead of the address while the user types.
	const filter = createListFilter({
		query: () => view.query,
		send: (value) => navigate({ ...view, query: value }, openId, true)
	});

	/** The view and the position open go to the address; focus and scroll stay where they are. */
	function navigate(next: StockView, itemId: number | null, replace: boolean) {
		const search = stockSearch(next, itemId);
		if (search === stockSearch(view, openId)) return;
		void goto(search ? resolve(`stock?${search}`) : resolve('stock'), { replace, reset: false });
	}
	/** A chip, a column: a new entry of the history, with the text as typed. */
	function change(fn: (v: StockView) => StockView) {
		filter.cancel();
		navigate(fn({ ...view, query: filter.text }), openId, false);
	}

	// The rows: a new view loads anew. Without the right to see the stock nothing is asked.
	const list = createStockList();
	const query = $derived(stockViewToQuery(view));
	const queryKey = $derived(JSON.stringify(query));
	$effect(() => {
		void queryKey;
		if (!canView) return;
		untrack(() => list.load(query));
	});
	// A page more that failed keeps the rows; the toast offers to ask again.
	$effect(() => {
		const e = list.error;
		if (e && untrack(() => list.rows.length > 0)) {
			toast.show({
				message: `${t.loadFailed}: ${e.message}`,
				tone: 'danger',
				action: { label: ru.common.retry, run: () => list.loadMore() }
			});
		}
	});
	const filtered = $derived(view.query.trim() !== '' || view.restock);

	// The position open: the address says which; the cursor follows it.
	let activeId = $state<RowId | null>(null);
	$effect(() => {
		const id = openId;
		if (id !== null) untrack(() => (activeId = id));
	});
	function openItem(id: number | null) {
		if (id === openId) return;
		filter.cancel();
		navigate({ ...view, query: filter.text }, id, true);
	}

	let adding = $state(false);
	function created(item: StockItem) {
		list.load(query);
		filter.cancel();
		navigate({ ...view, query: filter.text }, item.id, false);
	}

	const itemId = (s: StockItem) => s.id;
	const COLUMNS = $derived(
		stockColumns({
			cells: {
				id: idCell,
				name: nameCell,
				quantity: quantityCell,
				price: priceCell,
				restock: restockCell,
				part_type: partTypeCell,
				device_type: deviceTypeCell,
				brand: brandCell,
				model: modelCell,
				color: colorCell
			}
		})
	);
	const dash = (v: string) => v || '—';
</script>

{#snippet idCell(s: StockItem)}<span class="num">{s.id}</span>{/snippet}
{#snippet nameCell(s: StockItem)}<span class="strong">{s.display_name}</span>{/snippet}
{#snippet quantityCell(s: StockItem)}
	<span class="quantity">
		<span class="num">{s.quantity}</span>
		{#if s.needs_restock}<Badge tone="warning">{t.restockBadge}</Badge>{/if}
	</span>
{/snippet}
{#snippet priceCell(s: StockItem)}<span class="num">{formatMoney(s.price)}</span>{/snippet}
{#snippet restockCell(s: StockItem)}
	<span class="num">{s.restock_threshold > 0 ? s.restock_threshold : '—'}</span>
{/snippet}
{#snippet partTypeCell(s: StockItem)}{dash(s.part_type)}{/snippet}
{#snippet deviceTypeCell(s: StockItem)}{dash(s.device_type)}{/snippet}
{#snippet brandCell(s: StockItem)}{dash(s.brand)}{/snippet}
{#snippet modelCell(s: StockItem)}{dash(s.model)}{/snippet}
{#snippet colorCell(s: StockItem)}{dash(s.color)}{/snippet}

{#snippet empty()}
	{#if list.error}
		<EmptyState title={t.loadFailed} description={list.error.message}>
			{#snippet actions()}
				<Button size="sm" onclick={() => list.load(query)}>{ru.common.retry}</Button>
			{/snippet}
		</EmptyState>
	{:else if view.restock && view.query.trim() === ''}
		<EmptyState title={t.noRestock} description={t.noRestockHint} />
	{:else if filtered}
		<EmptyState title={t.nothingFound} description={t.nothingFoundHint}>
			{#snippet actions()}
				<Button
					size="sm"
					onclick={() => {
						filter.text = '';
						change(() => ({ ...defaultStockView(), sort: { ...view.sort } }));
					}}>{t.resetFilter}</Button
				>
			{/snippet}
		</EmptyState>
	{:else}
		<EmptyState title={t.empty} description={canCreate ? t.emptyHint : undefined} />
	{/if}
{/snippet}

{#snippet filterField()}
	<div class="filter">
		<TextInput
			type="search"
			ariaLabel={t.filter}
			placeholder={t.filterPlaceholder}
			autocomplete="off"
			bind:value={filter.text}
			bind:ref={filter.ref}
			oninput={filter.typed}
		>
			{#snippet prefix()}<Search size={16} aria-hidden="true" />{/snippet}
			{#snippet suffix()}<Kbd shortcut="/" />{/snippet}
		</TextInput>
	</div>
{/snippet}

{#snippet chips()}
	<div class="chips">
		<FilterChips
			label={t.chips}
			items={[
				{ id: 'all', label: t.all },
				{ id: 'restock', label: t.restock }
			]}
			bind:selected={() => shownStockChips(view), (next) => change((v) => withStockChips(v, next))}
		/>
		<HelpHint title={t.restock} text={ru.help.stock.restock} />
	</div>
{/snippet}

{#snippet addButton()}
	{#if canCreate}
		<Button variant="primary" icon={Plus} onclick={() => (adding = true)}>{t.add}</Button>
	{/if}
{/snippet}

<svelte:head><title>{t.title}</title></svelte:head>

{#if !canView}
	<div class="stock">
		<h1 class="title">{t.title}</h1>
		<EmptyState icon={Warehouse} title={t.noAccess} description={t.noAccessHint} />
	</div>
{:else}
	<div class="stock" class:phone={display.phone}>
		<h1 class="title">{t.title}</h1>
		{#if display.phone}
			<div class="toolbar">{@render filterField()}</div>
			{@render chips()}
			<div class="toolbar">{@render addButton()}</div>
		{:else}
			<div class="toolbar">
				{@render filterField()}
				{@render chips()}
				<span class="count num" aria-live="polite">
					{#if !list.loading || list.rows.length > 0}{countLabel(list.total, t.forms)}{/if}
				</span>
				{@render addButton()}
			</div>
		{/if}

		{#if display.phone}
			<InfiniteList
				items={list.rows}
				key={itemId}
				loading={list.loading}
				onendreached={() => list.loadMore()}
				{empty}
			>
				{#snippet item(s)}
					<ListCard
						number={s.display_name}
						title={t.pieces(s.quantity)}
						subtitle={formatMoney(s.price)}
						status={s.needs_restock ? { name: t.restockBadge, color: 'orange' } : undefined}
						active={openId === s.id}
						onclick={() => openItem(s.id)}
					/>
				{/snippet}
			</InfiniteList>
		{:else}
			<div class="table">
				<DataTable
					rows={list.rows}
					columns={COLUMNS}
					rowId={itemId}
					storageKey="stock"
					ariaLabel={t.title}
					sort={view.sort}
					onsortchange={(sort) => change((v) => ({ ...v, sort }))}
					bind:activeId
					onopen={(s) => openItem(s.id)}
					loading={list.loading}
					onendreached={() => list.loadMore()}
					{empty}
				/>
			</div>
		{/if}
	</div>

	<StockItemPanel itemId={openId} onclose={() => openItem(null)} />
	{#if canCreate}<StockAddDialog bind:open={adding} oncreated={created} />{/if}
{/if}

<style>
	.stock {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		height: 100%;
		padding: var(--space-4);
	}
	.stock.phone {
		height: auto;
		min-height: 100%;
		padding: var(--space-3);
	}
	/* The heading is for screen readers and the window title; the screen shows the list itself. */
	.title {
		position: absolute;
		width: 1px;
		height: 1px;
		margin: -1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.toolbar {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}
	.filter {
		flex: 1;
		min-width: 0;
		max-width: 36rem;
	}
	.chips {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
	}
	.phone .filter {
		max-width: none;
	}
	.count {
		flex: none;
		margin-left: auto;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.table {
		position: relative;
		flex: 1;
		min-height: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		overflow: hidden;
	}
	.strong {
		font-weight: 500;
	}
	.quantity {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
</style>
