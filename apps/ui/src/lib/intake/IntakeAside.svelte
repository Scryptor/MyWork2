<!-- The right column of the intake (spec 6.5): what is left to fill, the client's past orders and the
     orders with the same IMEI, with the warranty check. -->
<script lang="ts">
	import type { OrderListItem } from '$api/OrderListItem';
	import type { OrderPage } from '$api/OrderPage';
	import { getOrder, listOrders } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { formatDay, formatDate } from '#lib/format/date.js';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';
	import { deviceOf } from '#lib/orders/view.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Section from '#lib/ui/Section.svelte';
	import StatusBadge from '#lib/ui/StatusBadge.svelte';
	import {
		REQUIRED_LABELS,
		serialHits,
		serialQuery,
		underWarranty,
		type RequiredKey
	} from './draft.js';

	let {
		missing,
		wrong,
		clientId,
		serial,
		onopen
	}: {
		missing: RequiredKey[];
		/** Labels of the money fields holding text that is not an amount. */
		wrong: string[];
		clientId: number | null;
		serial: string;
		/** An order of the column was clicked. */
		onopen: (id: number) => void;
	} = $props();

	const t = ru.intake;
	const SHOWN = 5;
	/** Pause after the last character of the serial before the check. */
	const LOOKUP_MS = 300;

	let past = $state.raw<{ clientId: number; page: OrderPage } | null>(null);
	$effect(() => {
		const id = clientId;
		if (id === null) return;
		const controller = new AbortController();
		listOrders({ client_id: id, limit: SHOWN }, controller.signal).then(
			(page) => (past = { clientId: id, page }),
			// Aborted, or no connection: the column is a help, the intake works without it.
			() => {}
		);
		return () => controller.abort();
	});
	const pastOrders = $derived(past !== null && past.clientId === clientId ? past.page : null);

	interface Seen {
		order: OrderListItem;
		/** «на гарантии до …»: the end of a warranty still running. */
		until: string | null;
	}
	let seen = $state.raw<{ query: string; hits: Seen[] } | null>(null);
	const query = $derived(serialQuery(serial));
	$effect(() => {
		const q = query;
		if (q === null) return;
		const controller = new AbortController();
		const timer = setTimeout(() => void check(q, controller.signal), LOOKUP_MS);
		return () => {
			clearTimeout(timer);
			controller.abort();
		};
	});
	const seenHits = $derived(seen !== null && seen.query === query ? seen.hits : []);

	async function check(q: string, signal: AbortSignal) {
		let orders: OrderListItem[];
		try {
			orders = serialHits((await listOrders({ query: q, limit: SHOWN }, signal)).items, q);
		} catch {
			return;
		}
		const now = serverClock.now();
		// The list has no warranty term: the issued orders are read in full.
		const hits = await Promise.all(
			orders.map(async (order): Promise<Seen> => {
				if (order.issued_at === null) return { order, until: null };
				const full = await getOrder(order.id, signal).catch(() => null);
				const until = full && underWarranty(order.issued_at, full.warranty_term, now);
				return { order, until: until ? formatDay(until) : null };
			})
		);
		if (!signal.aborted) seen = { query: q, hits };
	}

	function seenNote(s: Seen): string {
		const issued = s.order.issued_at;
		if (issued === null) return t.inService;
		return [t.issued(formatDate(issued)), s.until ? t.warrantyUntil(s.until) : '']
			.filter(Boolean)
			.join(', ');
	}
</script>

<aside class="aside" aria-label={t.summary}>
	<Card padding="sm">
		<Section title={t.summary}>
			{#if missing.length > 0}
				<p class="missing">{t.missing(missing.map((k) => REQUIRED_LABELS[k]).join(', '))}</p>
			{/if}
			{#if wrong.length > 0}
				<p class="missing">{t.wrongSum(wrong.join(', '))}</p>
			{/if}
			{#if missing.length === 0 && wrong.length === 0}
				<p class="ready">{t.ready}</p>
			{/if}
		</Section>
	</Card>

	{#if seenHits.length > 0}
		<Callout tone="warning" title={t.seenSerial}>
			<ul class="orders" aria-label={t.seenSerial}>
				{#each seenHits as s (s.order.id)}
					<li>
						<button type="button" class="order" onclick={() => onopen(s.order.id)}>
							<span class="num">№ {s.order.id}</span>
							<span>{deviceOf(s.order)}</span>
							<span class="muted">{t.received(formatDate(s.order.received_at))}</span>
						</button>
						<span class="mark" class:warranty={s.until !== null}>{seenNote(s)}</span>
					</li>
				{/each}
			</ul>
		</Callout>
	{/if}

	{#if clientId !== null && pastOrders !== null && pastOrders.items.length > 0}
		<Card padding="sm">
			<Section title={t.pastOrders}>
				<ul class="orders" aria-label={t.pastOrders}>
					{#each pastOrders.items as o (o.id)}
						{@const status = app.statusById(o.status_id)}
						<li>
							<button type="button" class="order" onclick={() => onopen(o.id)}>
								<span class="num">№ {o.id} · {formatDate(o.received_at)}</span>
								<span>{deviceOf(o)}</span>
							</button>
							{#if status}<StatusBadge name={status.name} color={status.color} size="sm" />{/if}
						</li>
					{/each}
				</ul>
				{#if pastOrders.total > pastOrders.items.length}
					<p class="muted">{t.pastTotal(countLabel(pastOrders.total, ru.orders.forms))}</p>
				{/if}
			</Section>
		</Card>
	{/if}
</aside>

<style>
	.aside {
		position: sticky;
		top: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.aside p {
		margin: 0;
	}
	.missing {
		color: var(--warning-text);
	}
	.ready {
		color: var(--success-text);
	}
	.muted {
		color: var(--text-3);
	}
	.orders {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.orders li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		font-size: var(--text-sm);
	}
	.order {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-2);
		padding: 0;
		border: 0;
		background: none;
		color: inherit;
		font: inherit;
		text-align: start;
		text-decoration: underline;
		text-underline-offset: 2px;
		cursor: pointer;
	}
	.mark.warranty {
		font-weight: 600;
	}
	@media (max-width: 899px) {
		.aside {
			position: static;
		}
	}
</style>
