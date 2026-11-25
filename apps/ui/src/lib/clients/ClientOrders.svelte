<!-- «Заказы клиента» of the client panel (spec 6.6): the newest orders, issued too, as № · date · device ·
     status. A click opens the order's panel; orders changed or accepted meanwhile come again. More than
     fit here: «все» shows them on «Заказы». The pointer resting on an order shows its brief (device,
     malfunction, money, issued); a phone or a touch screen has no hover, and its row shows the total and
     the day issued under the device. -->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { OrderListItem } from '$api/OrderListItem';
	import { errorMessage } from '#lib/api/client.js';
	import { listOrders } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { display } from '#lib/design/prefs.svelte.js';
	import { formatFull, formatRelative } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import { onOrderChanged } from '#lib/orders/events.js';
	import { withOrder } from '#lib/orders/list.svelte.js';
	import { deviceOf } from '#lib/orders/view.js';
	import Button from '#lib/ui/Button.svelte';
	import HoverCard from '#lib/ui/HoverCard.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import StatusBadge from '#lib/ui/StatusBadge.svelte';
	import { orderBrief, phoneLine } from './brief.js';

	/** The orders shown at most: the newest ones. */
	const LIMIT = 50;

	let { clientId, onopen }: { clientId: number; onopen: (id: number) => void } = $props();

	const t = ru.client;
	let orders = $state.raw<OrderListItem[] | null>(null);
	let total = $state(0);
	let failure = $state<string | null>(null);
	let token = 0;

	async function load(id: number) {
		const mine = ++token;
		failure = null;
		try {
			const page = await listOrders({
				client_id: id,
				sort: 'received_at',
				descending: true,
				limit: LIMIT
			});
			if (mine !== token) return;
			orders = page.items;
			total = page.total;
		} catch (e) {
			if (mine !== token) return;
			failure = errorMessage(e);
		}
	}

	$effect(() => {
		orders = null;
		void load(clientId);
	});

	// An order of this client accepted («Новый заказ») comes in; one changed (its status in the order
	// panel, its money) is updated in place, the brief too.
	$effect(() =>
		onOrderChanged((o, change) => {
			if (o.client_id !== clientId) return;
			if (change === 'created') return void load(clientId);
			const at = orders?.findIndex((r) => r.id === o.id) ?? -1;
			const row = orders?.[at];
			if (!orders || !row || o.version < row.version) return;
			orders = orders.with(at, withOrder(row, o));
		})
	);
</script>

<!-- `props`: of the hover card that the row is the trigger of; none on a phone. -->
{#snippet row(o: OrderListItem, props: Record<string, unknown>)}
	{@const s = app.statusById(o.status_id)}
	{@const second = phoneLine(o, s?.kind)}
	<button {...props} type="button" class="order" onclick={() => onopen(o.id)}>
		<span class="number num">№ {o.id}</span>
		<span class="when num" title={formatFull(o.received_at)}
			>{formatRelative(o.received_at, serverClock.now())}</span
		>
		<!-- A long device is cut in the narrow panel: the whole of it on hover. -->
		<span class="device">
			<span class="device-name" title={deviceOf(o)}>{deviceOf(o)}</span>
			{#if second}<span class="second num">{second}</span>{/if}
		</span>
		{#if s}<StatusBadge name={s.name} color={s.color} size="sm" />{/if}
	</button>
{/snippet}

<Section title={t.orders}>
	{#if failure}
		<p class="muted">{t.ordersFailed}: {failure}</p>
		<div>
			<Button size="sm" onclick={() => void load(clientId)}>{ru.common.retry}</Button>
		</div>
	{:else if orders === null}
		<div class="loading" aria-busy="true">
			<Skeleton height="2.5rem" />
			<Skeleton height="2.5rem" />
		</div>
	{:else if orders.length === 0}
		<p class="muted">{t.noOrders}</p>
	{:else}
		<ul class="orders" aria-label={t.orders}>
			{#each orders as o (o.id)}
				<li>
					{#if display.phone}
						{@render row(o, {})}
					{:else}
						<HoverCard>
							{#snippet trigger(props)}{@render row(o, props)}{/snippet}
							{@const brief = orderBrief(o, app.statusById(o.status_id))}
							<div class="brief">
								<strong class="num">{brief.title}</strong>
								{#each brief.lines as line, i (i)}<span>{line}</span>{/each}
							</div>
						</HoverCard>
					{/if}
				</li>
			{/each}
		</ul>
		{#if total > orders.length}
			<p class="muted">
				{t.moreOrders(orders.length, total)} ·
				<a href={resolve(`orders?client=${clientId}&chips=all`)}>{t.allOrders}</a>
			</p>
		{/if}
	{/if}
</Section>

<style>
	.orders {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		overflow: hidden;
	}
	li + li {
		border-top: 1px solid var(--border);
	}
	.order {
		display: grid;
		grid-template-columns: auto auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		min-height: var(--control-lg);
		padding: var(--space-1-5) var(--space-3);
		border: 0;
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: var(--text-md);
		text-align: left;
		cursor: pointer;
	}
	@media (hover: hover) {
		.order:hover {
			background: var(--surface-3);
		}
	}
	.number {
		font-weight: 600;
	}
	.when {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.device {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.device-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* The brief as a line of the row where there is no hover to open its card: a phone, a touch screen
	   of any width (a tablet). A tap opens the order. */
	.second {
		display: none;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	@media (hover: none) {
		.second {
			display: block;
		}
	}
	.brief {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
	}
	a {
		color: var(--accent);
	}
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.loading {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	:global([data-width='xs']) .order {
		grid-template-columns: auto minmax(0, 1fr) auto;
	}
	:global([data-width='xs']) .when {
		display: none;
	}
	:global([data-width='xs']) .second {
		display: block;
	}
</style>
