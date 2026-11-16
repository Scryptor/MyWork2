<script lang="ts">
	import Printer from '@lucide/svelte/icons/printer';
	import { onMount } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import type { OrderListItem } from '$api/OrderListItem';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { DEMO_NOW, DEMO_POINTS, DEMO_STATUSES, makeOrders } from '#lib/demo/fake.js';
	import { sortOrders } from '#lib/demo/orders.js';
	import { deviceOf, estimateText } from '#lib/orders/view.js';
	import {
		countLabel,
		displayName,
		formatFull,
		formatMoney,
		formatPhone,
		formatRelative,
		shortName
	} from '#lib/format/index.js';
	import {
		Button,
		DataTable,
		FlagIcons,
		InfiniteList,
		ListCard,
		SelectionBar,
		StatusBadge,
		toast,
		type Column,
		type RowId,
		type SortState
	} from '#lib/ui/index.js';

	// 10 000 fictional orders arrive like from a server: sorted there, in pages.
	const ORDERS = makeOrders(10000);
	const TABLE_PAGE = 2500;
	const LIST_PAGE = 30;
	const LATENCY_MS = 200;

	const statusById = new Map(DEMO_STATUSES.map((s) => [s.id, s]));
	const pointById = new Map(DEMO_POINTS.map((p) => [p.id, p.name]));
	// The demo data is made around DEMO_NOW: «сегодня» and «вчера» stay the same days.
	const now = DEMO_NOW;
	const phone = new MediaQuery(PHONE_QUERY);

	const orderId = (o: OrderListItem) => o.id;
	const pointOf = (o: OrderListItem) =>
		o.service_point_id === null ? '' : (pointById.get(o.service_point_id) ?? '');
	const sortContext = {
		statusOrder: (id: number) => statusById.get(id)?.sort_order ?? 0,
		pointName: (id: number | null) => (id === null ? '' : (pointById.get(id) ?? ''))
	};

	// Spec 6.1: sorted by «Принят», newest first.
	let sort = $state<SortState>({ columnId: 'received', descending: true });
	let loaded = $state(0);
	let loading = $state(false);
	let selected = $state(new Set<RowId>());
	let activeId = $state<RowId | null>(null);
	let opened = $state<number | null>(null);
	let flashIds = $state<RowId[]>([]);

	const sorted = $derived(sortOrders(ORDERS, sort, sortContext));
	const rows = $derived(sorted.slice(0, loaded));

	let timer: ReturnType<typeof setTimeout> | undefined;
	function loadMore() {
		if (loading || loaded >= ORDERS.length) return;
		loading = true;
		timer = setTimeout(() => {
			loaded = Math.min(ORDERS.length, loaded + (phone.current ? LIST_PAGE : TABLE_PAGE));
			loading = false;
		}, LATENCY_MS);
	}
	function onsortchange(s: SortState) {
		clearTimeout(timer);
		sort = s;
		// A list sorted anew is another data set: the selection does not carry over.
		selected = new Set();
		loaded = 0;
		loading = false;
		loadMore();
	}
	onMount(() => {
		loadMore();
		return () => clearTimeout(timer);
	});

	function open(o: OrderListItem) {
		activeId = o.id;
		opened = o.id;
	}
	/** Someone changed an order on another computer: a row other than the open one flashes. */
	function changeElsewhere() {
		const other = rows.find((o) => o.id !== activeId);
		if (other) flashIds = [other.id];
	}
	function printSelected() {
		toast.show({
			message: `Отправлено на печать: ${countLabel(selected.size, ['заказ', 'заказа', 'заказов'])}`
		});
	}

	const COLUMNS: Column<OrderListItem>[] = [
		{ id: 'id', header: '№', width: 84, sortable: true, cell: numberCell },
		{ id: 'received', header: 'Принят', width: 132, sortable: true, cell: receivedCell },
		{ id: 'client', header: 'Клиент', width: 200, minWidth: 120, sortable: true, cell: clientCell },
		{ id: 'device', header: 'Устройство', width: 180, sortable: true, cell: deviceCell },
		{ id: 'malfunction', header: 'Неисправность', width: 200, cell: malfunctionCell },
		{ id: 'status', header: 'Статус', width: 212, sortable: true, cell: statusCell },
		{ id: 'master', header: 'Мастер', width: 110, sortable: true, cell: masterCell },
		{ id: 'sum', header: 'Сумма', width: 140, align: 'end', sortable: true, cell: sumCell },
		{ id: 'flags', header: 'Отметки', width: 96, cell: flagsCell },
		{ id: 'phone', header: 'Телефон', width: 150, hiddenByDefault: true, cell: phoneCell },
		{
			id: 'type',
			header: 'Тип',
			width: 120,
			sortable: true,
			hiddenByDefault: true,
			cell: typeCell
		},
		{ id: 'serial', header: 'Серийный/IMEI', width: 170, hiddenByDefault: true, cell: serialCell },
		{
			id: 'point',
			header: 'Точка',
			width: 110,
			sortable: true,
			hiddenByDefault: true,
			cell: pointCell
		},
		{
			id: 'issued',
			header: 'Выдан',
			width: 132,
			sortable: true,
			hiddenByDefault: true,
			cell: issuedCell
		},
		{
			id: 'prepayment',
			header: 'Предоплата',
			width: 120,
			align: 'end',
			sortable: true,
			hiddenByDefault: true,
			cell: prepaymentCell
		},
		{ id: 'barcode', header: 'Штрихкод', width: 140, hiddenByDefault: true, cell: barcodeCell }
	];
</script>

{#snippet numberCell(o: OrderListItem)}{o.id}{/snippet}
{#snippet receivedCell(o: OrderListItem)}
	<span title={formatFull(o.received_at)}>{formatRelative(o.received_at, now)}</span>
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
	{@const s = statusById.get(o.status_id)}
	{#if s}<StatusBadge name={s.name} color={s.color} size="sm" />{/if}
{/snippet}
{#snippet masterCell(o: OrderListItem)}{o.master ? displayName(o.master) : '—'}{/snippet}
{#snippet sumCell(o: OrderListItem)}
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
{#snippet typeCell(o: OrderListItem)}{o.device_type}{/snippet}
{#snippet serialCell(o: OrderListItem)}{o.serial || '—'}{/snippet}
{#snippet pointCell(o: OrderListItem)}{pointOf(o) || '—'}{/snippet}
{#snippet issuedCell(o: OrderListItem)}
	{#if o.issued_at}
		<span title={formatFull(o.issued_at)}>{formatRelative(o.issued_at, now)}</span>
	{:else}
		<span class="muted">—</span>
	{/if}
{/snippet}
{#snippet prepaymentCell(o: OrderListItem)}
	{o.prepayment > 0 ? formatMoney(o.prepayment) : '—'}
{/snippet}
{#snippet barcodeCell(o: OrderListItem)}{o.barcode}{/snippet}

<p class="muted">
	10 000 вымышленных заказов приходят страницами, как с сервера. Клик или Enter открывает заказ, ↑↓
	двигают строку, пробел и Shift+клик выделяют. Ширину колонки меняет край заголовка, порядок и
	видимость — «Колонки» в углу шапки. На телефоне — карточки.
</p>
<div class="row">
	{#if !phone.current}
		<Button size="sm" onclick={changeElsewhere}>Изменить на другом компьютере</Button>
	{/if}
	<span class="muted" aria-live="polite">
		{opened === null ? 'Ничего не открыто' : `Открыт № ${opened}`}
	</span>
</div>

{#if phone.current}
	<InfiniteList items={rows} key={orderId} {loading} onendreached={loadMore}>
		{#snippet item(o)}
			{@const s = statusById.get(o.status_id)}
			<ListCard
				number="№ {o.id}"
				title="{deviceOf(o)} · {o.malfunction}"
				subtitle={shortName(o.client_name)}
				meta={formatRelative(o.received_at, now)}
				status={s ? { name: s.name, color: s.color } : undefined}
				flags={{ needCall: o.need_call, needParts: o.need_parts }}
				active={activeId === o.id}
				onclick={() => open(o)}
			/>
		{/snippet}
	</InfiniteList>
{:else}
	<div class="frame">
		<DataTable
			{rows}
			columns={COLUMNS}
			rowId={orderId}
			storageKey="design.orders"
			ariaLabel="Заказы"
			{sort}
			{onsortchange}
			selectable
			bind:selected
			bind:activeId
			onopen={open}
			{loading}
			onendreached={loadMore}
			{flashIds}
		/>
		<div class="selection">
			<SelectionBar count={selected.size} onclear={() => (selected = new Set())}>
				<Button size="sm" variant="ghost" icon={Printer} onclick={printSelected}>Печать</Button>
			</SelectionBar>
		</div>
	</div>
{/if}

<style>
	.muted {
		color: var(--text-3);
	}
	p.muted {
		max-width: 48rem;
	}
	.strong {
		font-weight: 500;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-bottom: var(--space-3);
	}
	.frame {
		position: relative;
		height: 30rem;
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
</style>
