<!-- «Клиенты» (spec 6.6, 6.12): the list filter, «Все» / «Похожие», the table (cards on a phone) and the
     client panel. The view and the client open live in the address: view.ts reads and writes them. -->
<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import Search from '@lucide/svelte/icons/search';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Client } from '$api/Client';
	import type { ClientListItem } from '$api/ClientListItem';
	import { app } from '#lib/app/context.svelte.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import ClientPanel from '#lib/clients/ClientPanel.svelte';
	import MergeDialog from '#lib/clients/MergeDialog.svelte';
	import ExportClientsMenu from '#lib/export/ExportClientsMenu.svelte';
	import { clientColumns, phonesText } from '#lib/clients/columns.js';
	import { createClientList } from '#lib/clients/list.svelte.js';
	import {
		clientsSearch,
		clientsViewFromSearch,
		clientsViewToQuery,
		defaultClientsView,
		openClientId,
		shownChips,
		withChips,
		type ClientsView
	} from '#lib/clients/view.js';
	import { createListFilter } from '#lib/app/list-filter.svelte.js';
	import { countLabel, displayName, formatFull, formatRelative } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import { onOrderChanged } from '#lib/orders/events.js';
	import Button from '#lib/ui/Button.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import FilterChips from '#lib/ui/FilterChips.svelte';
	import FlagIcons from '#lib/ui/FlagIcons.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import InfiniteList from '#lib/ui/InfiniteList.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';
	import ListCard from '#lib/ui/ListCard.svelte';
	import DataTable from '#lib/ui/table/DataTable.svelte';
	import SelectionBar from '#lib/ui/table/SelectionBar.svelte';
	import type { RowId } from '#lib/ui/table/types.js';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.clients;

	const view = $derived(clientsViewFromSearch(page.url.searchParams));
	const openId = $derived(openClientId(page.url.searchParams));

	// The list filter: its text is ahead of the address while the user types.
	const filter = createListFilter({
		query: () => view.query,
		send: (value) => navigate({ ...view, query: value }, openId, true)
	});

	/** The view and the client open go to the address; focus and scroll stay where they are. */
	function navigate(next: ClientsView, clientId: number | null, replace: boolean) {
		const search = clientsSearch(next, clientId);
		if (search === clientsSearch(view, openId)) return;
		void goto(search ? resolve(`clients?${search}`) : resolve('clients'), {
			replace,
			reset: false
		});
	}
	/** A chip, a column: a new entry of the history, with the text as typed. */
	function change(fn: (v: ClientsView) => ClientsView) {
		filter.cancel();
		navigate(fn({ ...view, query: filter.text }), openId, false);
	}

	// The rows: a new view loads anew and drops the selection.
	const list = createClientList();
	const query = $derived(clientsViewToQuery(view));
	const queryKey = $derived(JSON.stringify(query));
	let selected = $state.raw(new Set<RowId>());
	$effect(() => {
		void queryKey;
		untrack(() => {
			list.load(query);
			selected = new Set();
		});
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
	// A new order changes the count and the last order of its client: that row only, no page is read again.
	$effect(() =>
		onOrderChanged((o, change) => {
			if (change === 'created') list.orderCreated(o);
		})
	);
	const filtered = $derived(view.query.trim() !== '' || view.duplicates);

	// The client panel: the address says which client is open; leaving one saves its edits first.
	let panel = $state<ReturnType<typeof ClientPanel> | null>(null);
	let activeId = $state<RowId | null>(null);
	$effect(() => {
		const id = openId;
		if (id !== null) untrack(() => (activeId = id));
	});
	async function openClient(id: number | null) {
		if (id === openId) return;
		if (panel && !(await panel.leave())) {
			// The panel stays on its client when the edits there cannot be saved: so does the cursor.
			if (openId !== null) activeId = openId;
			return;
		}
		filter.cancel();
		navigate({ ...view, query: filter.text }, id, true);
	}

	// The merge: the clients are read afresh by the dialog; the panel's own edits are saved before.
	const MERGE_MAX = 10;
	let mergeIds = $state.raw<number[]>([]);
	let mergeCounts = $state.raw<Record<number, number>>({});
	let mergeOpen = $state(false);
	const canMerge = $derived(app.can('clients_merge'));
	async function startMerge(ids: number[], counts: Record<number, number> = {}) {
		if (panel && !(await panel.leave())) return;
		// The counts of orders come from the list rows (or from the search that found the client).
		const known = { ...counts };
		for (const id of ids) {
			const row = list.rows.find((r) => r.id === id);
			if (known[id] === undefined && row) known[id] = row.orders_count;
		}
		mergeCounts = known;
		mergeIds = ids;
		mergeOpen = true;
	}
	function merged(kept: Client) {
		selected = new Set();
		list.load(query);
		toast.show({ message: ru.merge.done, tone: 'success' });
		if (kept.id === openId) panel?.reload();
		else void openClient(kept.id);
	}
	const orderBack = (id: number) => ({
		label: ru.client.backToClient,
		href: resolve(`clients?${clientsSearch(view, id)}`)
	});

	const clientId = (c: ClientListItem) => c.id;
	const COLUMNS = $derived(
		clientColumns({
			cells: {
				name: nameCell,
				phones: phonesCell,
				orders: ordersCell,
				last: lastCell,
				source: sourceCell,
				flags: flagsCell
			},
			sortable: !view.duplicates
		})
	);
</script>

{#snippet nameCell(c: ClientListItem)}
	<span class="strong">{displayName(c.full_name) || ru.client.noName}</span>
{/snippet}
{#snippet phonesCell(c: ClientListItem)}<span class="num">{phonesText(c.phones) || '—'}</span
	>{/snippet}
{#snippet ordersCell(c: ClientListItem)}<span class="num">{c.orders_count}</span>{/snippet}
{#snippet lastCell(c: ClientListItem)}
	{#if c.last_order_at}
		<span title={formatFull(c.last_order_at)}
			>{formatRelative(c.last_order_at, serverClock.now())}</span
		>
	{:else}
		<span class="muted">—</span>
	{/if}
{/snippet}
{#snippet sourceCell(c: ClientListItem)}{c.source || '—'}{/snippet}
{#snippet flagsCell(c: ClientListItem)}
	<FlagIcons needCall={false} needParts={false} blacklisted={c.blacklisted} focusable={false} />
{/snippet}

{#snippet empty()}
	{#if list.error}
		<EmptyState title={t.loadFailed} description={list.error.message}>
			{#snippet actions()}
				<Button size="sm" onclick={() => list.load(query)}>{ru.common.retry}</Button>
			{/snippet}
		</EmptyState>
	{:else if view.duplicates && view.query.trim() === ''}
		<EmptyState title={t.noSimilar} />
	{:else if filtered}
		<EmptyState title={t.nothingFound} description={t.nothingFoundHint}>
			{#snippet actions()}
				<Button
					size="sm"
					onclick={() => {
						filter.text = '';
						change(() => ({ ...defaultClientsView(), sort: { ...view.sort } }));
					}}>{t.resetFilter}</Button
				>
			{/snippet}
		</EmptyState>
	{:else}
		<EmptyState title={t.empty} description={t.emptyHint} />
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
				{ id: 'similar', label: t.similar }
			]}
			bind:selected={() => shownChips(view), (next) => change((v) => withChips(v, next))}
		/>
		<HelpHint title={t.similar} text={ru.help.clients.similar} />
	</div>
{/snippet}

<svelte:head><title>{t.title}</title></svelte:head>

<div class="clients" class:phone={display.phone}>
	<h1 class="title">{t.title}</h1>
	{#if display.phone}
		<div class="toolbar">
			{@render filterField()}<ExportClientsMenu {query} />
		</div>
		{@render chips()}
	{:else}
		<div class="toolbar">
			{@render filterField()}
			{@render chips()}
			<ExportClientsMenu {query} />
			<span class="count num" aria-live="polite">
				{#if !list.loading || list.rows.length > 0}{countLabel(list.total, t.forms)}{/if}
			</span>
		</div>
	{/if}

	{#if display.phone}
		<InfiniteList
			items={list.rows}
			key={clientId}
			loading={list.loading}
			onendreached={() => list.loadMore()}
			{empty}
		>
			{#snippet item(c)}
				<ListCard
					number={displayName(c.full_name) || ru.client.noName}
					title={phonesText(c.phones) || '—'}
					subtitle={c.orders_count > 0 ? countLabel(c.orders_count, ru.orders.forms) : t.noOrders}
					meta={c.last_order_at
						? t.lastOrder(formatRelative(c.last_order_at, serverClock.now()))
						: undefined}
					status={c.blacklisted ? { name: ru.flags.blacklisted, color: 'red' } : undefined}
					active={openId === c.id}
					onclick={() => void openClient(c.id)}
				/>
			{/snippet}
		</InfiniteList>
	{:else}
		<div class="table">
			<DataTable
				rows={list.rows}
				columns={COLUMNS}
				rowId={clientId}
				storageKey="clients"
				ariaLabel={t.title}
				sort={view.duplicates ? { columnId: 'name', descending: false } : view.sort}
				onsortchange={(sort) => change((v) => ({ ...v, sort }))}
				selectable
				bind:selected
				bind:activeId
				onopen={(c) => void openClient(c.id)}
				loading={list.loading}
				onendreached={() => list.loadMore()}
				{empty}
			/>
			<div class="selection">
				<SelectionBar count={selected.size} onclear={() => (selected = new Set())}>
					{#if canMerge && selected.size >= 2 && selected.size <= MERGE_MAX}
						<Button size="sm" onclick={() => void startMerge([...selected].map((id) => Number(id)))}
							>{ru.merge.action}</Button
						>
					{/if}
				</SelectionBar>
			</div>
		</div>
	{/if}
</div>

<ClientPanel
	bind:this={panel}
	clientId={openId}
	{orderBack}
	onclose={() => {
		filter.cancel();
		navigate({ ...view, query: filter.text }, null, true);
	}}
	onopenclient={(id) => void openClient(id)}
	onchanged={(c) => list.replace(c)}
	onmerge={(ids, counts) => void startMerge(ids, counts)}
/>

<MergeDialog bind:open={mergeOpen} ids={mergeIds} counts={mergeCounts} onmerged={merged} />

<style>
	.clients {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		height: 100%;
		padding: var(--space-4);
	}
	.clients.phone {
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
	.selection {
		position: absolute;
		bottom: var(--space-3);
		left: 50%;
		z-index: var(--z-sticky);
		max-width: calc(100% - 2 * var(--space-3));
		transform: translateX(-50%);
	}
	.strong {
		font-weight: 500;
	}
	.muted {
		color: var(--text-3);
	}
</style>
