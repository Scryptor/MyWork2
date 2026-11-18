<!-- The main screen «Заказы» (spec 6.1, 6.12): quick filters, the list filter, «Фильтры ▾», the table
     (cards on a phone). The view lives in the address: view.ts reads and writes it. -->
<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Clock from '@lucide/svelte/icons/clock';
	import Search from '@lucide/svelte/icons/search';
	import X from '@lucide/svelte/icons/x';
	import { tick, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { OrderListItem } from '$api/OrderListItem';
	import type { Status } from '$api/Status';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { getClient } from '#lib/api/clients.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { summary } from '#lib/app/summary.svelte.js';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import {
		countLabel,
		displayName,
		formatFull,
		formatMoney,
		formatPhone,
		formatRelative,
		shortName
	} from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import BulkIssuedDateDialog from '#lib/orders/BulkIssuedDateDialog.svelte';
	import { performBulk, type BulkChange } from '#lib/orders/bulk.js';
	import { orderColumns } from '#lib/orders/columns.js';
	import { onOrderChanged } from '#lib/orders/events.js';
	import FiltersMenu from '#lib/orders/FiltersMenu.svelte';
	import { createOrderList } from '#lib/orders/list.svelte.js';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import QuickFilters from '#lib/orders/QuickFilters.svelte';
	import { savedViews } from '#lib/orders/saved-views.svelte.js';
	import {
		appliedFilters,
		deviceOf,
		displayedChips,
		estimateText,
		hasFilters,
		isOverdueDiagnostics,
		quickFilterItems,
		resetFilters,
		viewFromSearch,
		viewToQuery,
		viewToSearch,
		withChips,
		withoutFilter,
		type OrderFilters,
		type OrdersView
	} from '#lib/orders/view.js';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import FlagIcons from '#lib/ui/FlagIcons.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import InfiniteList from '#lib/ui/InfiniteList.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';
	import ListCard from '#lib/ui/ListCard.svelte';
	import StatusBadge from '#lib/ui/StatusBadge.svelte';
	import DataTable from '#lib/ui/table/DataTable.svelte';
	import SelectionBar from '#lib/ui/table/SelectionBar.svelte';
	import type { RowId } from '#lib/ui/table/types.js';
	import TextInput from '#lib/ui/TextInput.svelte';
	import Button from '#lib/ui/Button.svelte';
	import VisuallyHidden from '#lib/ui/VisuallyHidden.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.orders;
	/** The list filter asks the server once typing pauses for this long. */
	const FILTER_MS = 250;

	const view = $derived(viewFromSearch(page.url.searchParams));

	// The list filter: its text is ahead of the address while the user types.
	let text = $state(untrack(() => view.query));
	let filterEl = $state<HTMLInputElement | null>(null);
	let typing: ReturnType<typeof setTimeout> | undefined;
	/** The text sent last from typing; its arrival in the address does not touch the field. */
	let sent: string | null = null;
	const viewQuery = $derived(view.query);
	$effect(() => {
		const q = viewQuery;
		untrack(() => {
			if (q === sent) {
				sent = null;
				return;
			}
			sent = null;
			if (q !== text) {
				clearTimeout(typing);
				text = q;
			}
		});
	});
	$effect(() => () => clearTimeout(typing));
	$effect(() => registerHotkey('/', () => filterEl?.focus()));

	/** The view goes to the address; focus and scroll stay where they are. */
	function navigate(next: OrdersView, replace: boolean) {
		const search = viewToSearch(next);
		if (search === viewToSearch(view)) return;
		void goto(search ? resolve(`orders?${search}`) : resolve('orders'), { replace, reset: false });
	}
	/** A click on a chip, a filter, a column: a new entry of the history, with the text as typed. */
	function change(fn: (v: OrdersView) => OrdersView) {
		clearTimeout(typing);
		navigate(fn({ ...view, query: text }), false);
	}
	function typed(value: string) {
		clearTimeout(typing);
		typing = setTimeout(() => {
			if (value === view.query) return;
			sent = value;
			navigate({ ...view, query: value }, true);
		}, FILTER_MS);
	}

	// The rows: a new view (or point, or statuses) loads anew and drops the selection.
	const list = createOrderList();
	const query = $derived(viewToQuery(view, app.statuses, app.point));
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

	// Counts of the quick filters and the sidebar: here, for another point, after edits.
	$effect(() => summary.refresh(app.point));
	$effect(() =>
		onOrderChanged((o, change) => {
			// A new order (intake) may belong to the view: the rows come again, the old ones stay meanwhile.
			if (change === 'created') list.load(query);
			else list.replace(o);
			summary.refresh(app.point);
		})
	);
	const quick = $derived(quickFilterItems(app.statuses, summary.current));
	const anyOrders = $derived(
		summary.current === null || summary.current.by_status.some((c) => c.count > 0)
	);

	// «Клиент: …» of a client chosen in Ctrl K.
	const clientId = $derived(view.clientId);
	let clientName = $state<string | null>(null);
	$effect(() => {
		const id = clientId;
		clientName = null;
		if (id === null) return;
		let current = true;
		getClient(id).then(
			(c) => {
				if (current) clientName = displayName(c.full_name);
			},
			() => {
				if (current) clientName = `№ ${id}`;
			}
		);
		return () => {
			current = false;
		};
	});
	const applied = $derived(
		appliedFilters(view, {
			statusName: (id) => app.statusById(id)?.name,
			pointName: (id) => app.points.find((p) => p.id === id)?.name,
			clientName
		})
	);
	const filtered = $derived(hasFilters(view));

	/** The filters come from the menu: the address may not have caught up with them yet. */
	async function saveView(name: string, filters: OrderFilters) {
		savedViews.add(name, viewToSearch({ ...view, query: text, filters }));
		// Toasts wait for the dialog to go.
		await tick();
		toast.show({ message: t.viewSaved(name.trim()), tone: 'success' });
	}

	// Bulk actions of the selection bar (spec 6.1): one order after another, one «Отменить» for all.
	const canEdit = $derived(app.can('orders_edit'));
	const anySelected = $derived(selected.size > 0);
	let bulkBusy = $state(false);
	let masters = $state.raw<string[]>([]);
	// The masters' dictionary, asked when a selection starts: settings may have changed.
	$effect(() => {
		if (!anySelected || !canEdit) return;
		let current = true;
		listDictionary('master').then(
			(items) => {
				if (current) masters = items.map((d) => d.value);
			},
			() => {
				// Without it «Снять мастера» stays.
			}
		);
		return () => {
			current = false;
		};
	});
	async function bulk(change: BulkChange) {
		const items = list.rows.filter((r) => selected.has(r.id));
		if (bulkBusy || items.length === 0) return;
		bulkBusy = true;
		try {
			await performBulk(items, change, {
				statusName: (id) => app.statusById(id)?.name ?? '',
				canClearIssuedAt: app.can('order_dates_edit')
			});
		} finally {
			bulkBusy = false;
			selected = new Set();
		}
	}
	/** An issued status asks first whether to stamp the issue date. */
	let issuedTo = $state.raw<Status | null>(null);
	let issuedOpen = $state(false);
	function bulkStatus(s: Status) {
		if (s.kind !== 'issued') return void bulk({ kind: 'status', statusId: s.id });
		issuedTo = s;
		issuedOpen = true;
	}
	const statusItems = $derived<MenuItem[]>(
		app.statuses
			.filter((s) => !s.archived)
			.map((s) => ({ label: s.name, onselect: () => bulkStatus(s) }))
	);
	const masterItems = $derived<MenuItem[]>([
		...masters.map((m) => ({ label: m, onselect: () => void bulk({ kind: 'master', master: m }) })),
		...(masters.length > 0 ? [{ separator: true as const }] : []),
		{ label: ru.bulk.clearMaster, onselect: () => void bulk({ kind: 'master', master: '' }) }
	]);
	const FLAGS = [
		{ flag: 'need_call', label: ru.flags.needCall },
		{ flag: 'need_parts', label: ru.flags.needParts }
	] as const;
	const markItems: MenuItem[] = FLAGS.flatMap(({ flag, label }) => [
		{
			label: ru.bulk.markOn(label),
			onselect: () => void bulk({ kind: 'flag', flag, value: true })
		},
		{
			label: ru.bulk.markOff(label),
			onselect: () => void bulk({ kind: 'flag', flag, value: false })
		}
	]);

	// The order open in the panel is the table's current row while it is in the list.
	let activeId = $state<RowId | null>(null);
	$effect(() => {
		const id = orderPanel.id;
		if (id !== null) untrack(() => (activeId = id));
	});
	function openOrder(id: number) {
		// The panel stays on its order when the edits there cannot be saved: so does the cursor.
		void orderPanel.open(id, { neighbors: () => list.rows.map((r) => r.id) }).then((ok) => {
			if (!ok && orderPanel.id !== null) activeId = orderPanel.id;
		});
	}

	const pointName = (id: number | null) =>
		id === null ? '' : (app.points.find((p) => p.id === id)?.name ?? '');
	const orderId = (o: OrderListItem) => o.id;
	const COLUMNS = orderColumns({
		cells: {
			id: numberCell,
			received: receivedCell,
			client: clientCell,
			device: deviceCell,
			malfunction: malfunctionCell,
			status: statusCell,
			master: masterCell,
			amount: amountCell,
			flags: flagsCell,
			phone: phoneCell,
			type: typeCell,
			serial: serialCell,
			point: pointCell,
			issued: issuedCell,
			prepayment: prepaymentCell,
			barcode: barcodeCell
		}
	});
</script>

{#snippet numberCell(o: OrderListItem)}{o.id}{/snippet}
{#snippet receivedCell(o: OrderListItem)}
	<span title={formatFull(o.received_at)}>{formatRelative(o.received_at, serverClock.now())}</span>
{/snippet}
{#snippet clientCell(o: OrderListItem)}
	{#if o.client_blacklisted}
		<FlagIcons needCall={false} needParts={false} blacklisted focusable={false} />
	{/if}
	<span class="strong">{displayName(o.client_name)}</span>
{/snippet}
{#snippet deviceCell(o: OrderListItem)}<span class="strong">{deviceOf(o)}</span>{/snippet}
{#snippet malfunctionCell(o: OrderListItem)}{o.malfunction}{/snippet}
{#snippet statusCell(o: OrderListItem)}
	{@const s = app.statusById(o.status_id)}
	{#if s}<StatusBadge name={s.name} color={s.color} size="sm" />{/if}
	{#if isOverdueDiagnostics(o, s?.kind, serverClock.now(), app.settings.diagnostics_overdue_days)}
		<span class="clock"
			><Clock size={14} aria-hidden="true" /><VisuallyHidden>{t.longDiagnostics}</VisuallyHidden
			></span
		>
	{/if}
{/snippet}
{#snippet masterCell(o: OrderListItem)}{o.master ? displayName(o.master) : '—'}{/snippet}
{#snippet amountCell(o: OrderListItem)}
	{#if o.final_price !== null}
		{formatMoney(o.final_price)}
	{:else if estimateText(o)}
		<span class="muted">≈ {estimateText(o)}</span>
	{:else}
		<span class="muted">—</span>
	{/if}
{/snippet}
{#snippet flagsCell(o: OrderListItem)}
	<FlagIcons needCall={o.need_call} needParts={o.need_parts} focusable={false} />
{/snippet}
{#snippet phoneCell(o: OrderListItem)}{formatPhone(o.client_phone)}{/snippet}
{#snippet typeCell(o: OrderListItem)}{o.device_type || '—'}{/snippet}
{#snippet serialCell(o: OrderListItem)}{o.serial || '—'}{/snippet}
{#snippet pointCell(o: OrderListItem)}{pointName(o.service_point_id) || '—'}{/snippet}
{#snippet issuedCell(o: OrderListItem)}
	{#if o.issued_at}
		<span title={formatFull(o.issued_at)}>{formatRelative(o.issued_at, serverClock.now())}</span>
	{:else}
		<span class="muted">—</span>
	{/if}
{/snippet}
{#snippet prepaymentCell(o: OrderListItem)}{o.prepayment > 0
		? formatMoney(o.prepayment)
		: '—'}{/snippet}
{#snippet barcodeCell(o: OrderListItem)}{o.barcode || '—'}{/snippet}

{#snippet empty()}
	{#if list.error}
		<EmptyState title={t.loadFailed} description={list.error.message}>
			{#snippet actions()}
				<Button size="sm" onclick={() => list.load(query)}>{ru.common.retry}</Button>
			{/snippet}
		</EmptyState>
	{:else if !anyOrders && !filtered}
		<EmptyState title={t.empty} description={t.emptyHint} />
	{:else if filtered}
		<EmptyState title={t.nothingFound} description={t.nothingFoundHint}>
			{#snippet actions()}
				<Button size="sm" onclick={() => change(resetFilters)}>{t.resetFilters}</Button>
			{/snippet}
		</EmptyState>
	{:else}
		<EmptyState title={t.nothingFound} />
	{/if}
{/snippet}

{#snippet bulkMenu(items: MenuItem[], label: string, text: string)}
	<DropdownMenu {items} {label}>
		{#snippet trigger(props)}
			<Button {...props} variant="ghost" size="sm" iconEnd={ChevronDown} busy={bulkBusy}
				>{text}</Button
			>
		{/snippet}
	</DropdownMenu>
{/snippet}

{#snippet filterField()}
	<div class="filter">
		<TextInput
			type="search"
			ariaLabel={t.filter}
			placeholder={t.filterPlaceholder}
			autocomplete="off"
			bind:value={text}
			bind:ref={filterEl}
			oninput={typed}
		>
			{#snippet prefix()}<Search size={16} aria-hidden="true" />{/snippet}
			{#snippet suffix()}<Kbd shortcut="/" />{/snippet}
		</TextInput>
	</div>
{/snippet}

{#snippet quickFilters()}
	<QuickFilters
		items={quick}
		selected={displayedChips(view)}
		onchange={(chips) => change((v) => withChips(v, chips))}
	/>
{/snippet}

{#snippet filtersMenu()}
	<FiltersMenu
		compact={display.phone}
		filters={view.filters}
		onapply={(filters) => change((v) => ({ ...v, filters }))}
		onsaveview={(name, filters) => void saveView(name, filters)}
	/>
{/snippet}

<div class="orders" class:phone={display.phone}>
	<h1 class="title">{t.title}</h1>
	{#if display.phone}
		<div class="toolbar">{@render filterField()}{@render filtersMenu()}</div>
		{@render quickFilters()}
	{:else}
		{@render quickFilters()}
		<div class="toolbar">
			{@render filterField()}
			{@render filtersMenu()}
			<span class="count num" aria-live="polite">
				{#if !list.loading || list.rows.length > 0}{countLabel(list.total, t.forms)}{/if}
			</span>
		</div>
	{/if}
	{#if applied.length > 0}
		<ul class="applied" aria-label={t.applied}>
			{#each applied as a (a.id)}
				<li class="chip">
					<span>{a.label}</span>
					<IconButton
						label={t.removeFilter(a.label)}
						icon={X}
						size="sm"
						onclick={() => change((v) => withoutFilter(v, a.id))}
					/>
				</li>
			{/each}
		</ul>
	{/if}

	{#if display.phone}
		<InfiniteList
			items={list.rows}
			key={orderId}
			loading={list.loading}
			onendreached={() => list.loadMore()}
			{empty}
		>
			{#snippet item(o)}
				{@const s = app.statusById(o.status_id)}
				<ListCard
					number="№ {o.id}"
					title={[deviceOf(o), o.malfunction].filter(Boolean).join(' · ')}
					subtitle={shortName(o.client_name)}
					meta={formatRelative(o.received_at, serverClock.now())}
					status={s ? { name: s.name, color: s.color } : undefined}
					flags={{ needCall: o.need_call, needParts: o.need_parts }}
					active={orderPanel.id === o.id}
					onclick={() => openOrder(o.id)}
				/>
			{/snippet}
		</InfiniteList>
	{:else}
		<div class="table">
			<DataTable
				rows={list.rows}
				columns={COLUMNS}
				rowId={orderId}
				storageKey="orders"
				ariaLabel={t.title}
				sort={view.sort}
				onsortchange={(sort) => change((v) => ({ ...v, sort }))}
				selectable
				bind:selected
				bind:activeId
				onopen={(o) => openOrder(o.id)}
				loading={list.loading}
				onendreached={() => list.loadMore()}
				{empty}
			/>
			<div class="selection">
				<!-- «Печать ▾» comes with plan 6. Without the right to edit orders only the count is shown. -->
				{#if canEdit}
					<SelectionBar count={selected.size} onclear={() => (selected = new Set())}>
						{@render bulkMenu(statusItems, ru.bulk.changeStatus, ru.bulk.status)}
						{@render bulkMenu(masterItems, ru.bulk.changeMaster, ru.bulk.master)}
						{@render bulkMenu(markItems, ru.bulk.changeMarks, ru.bulk.marks)}
					</SelectionBar>
				{:else}
					<SelectionBar count={selected.size} onclear={() => (selected = new Set())} />
				{/if}
			</div>
		</div>
	{/if}
</div>

<BulkIssuedDateDialog
	bind:open={issuedOpen}
	status={issuedTo?.name ?? ''}
	count={selected.size}
	onchoose={(stamp) => {
		if (issuedTo) void bulk({ kind: 'status', statusId: issuedTo.id, stamp });
	}}
/>

<style>
	.orders {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		height: 100%;
		padding: var(--space-4);
	}
	.orders.phone {
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
		gap: var(--space-2);
	}
	.filter {
		flex: 1;
		min-width: 0;
		max-width: 36rem;
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
	.applied {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1-5);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-0-5);
		padding-left: var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface-2);
		color: var(--text);
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
	.clock {
		display: inline-flex;
		margin-left: var(--space-1);
		vertical-align: middle;
		color: var(--warning-text);
	}
</style>
