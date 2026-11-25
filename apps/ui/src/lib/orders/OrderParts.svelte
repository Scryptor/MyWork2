<!-- «Запчасти» of an order (decisions 4–6): what was written off from the stock, with the sum. Whoever sees
     the order sees the list; writing off and giving back need «Изменение заказов» and «Просмотр склада». A
     deleted order only shows it. The panel and the full card both use this block. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import { resolve } from '$app/paths';
	import type { OrderPart } from '$api/OrderPart';
	import { errorMessage } from '#lib/api/client.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { formatRelative } from '#lib/format/date.js';
	import { formatMoney } from '#lib/format/money.js';
	import { ru } from '#lib/i18n/ru.js';
	import { stockItemHref } from '#lib/stock/view.js';
	import StockPickDialog from '#lib/stock/StockPickDialog.svelte';
	import Button from '#lib/ui/Button.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import Section from '#lib/ui/Section.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import type { OrderPartsState } from './parts.svelte.js';

	let { parts, orderDeleted = false }: { parts: OrderPartsState; orderDeleted?: boolean } =
		$props();

	const t = ru.orderParts;
	const canView = $derived(app.can('stock_view'));
	const canWrite = $derived(app.can('orders_edit') && canView && !orderDeleted);

	let picking = $state(false);
	let giving = $state<OrderPart | null>(null);

	/** The line of a row: «имя × n — сумма». */
	const meta = (p: OrderPart) =>
		p.legacy ? t.legacy : t.by(p.added_by_name, formatRelative(p.added_at, serverClock.now()));

	async function giveBack(p: OrderPart) {
		try {
			await parts.giveBack(p.id);
		} catch (e) {
			toast.show({ message: t.giveBackFailed(errorMessage(e)), tone: 'danger' });
		}
	}
</script>

<Section title={t.title} region>
	{#snippet actions()}
		{#if canWrite}
			<Button size="sm" icon={Plus} onclick={() => (picking = true)}>{t.add}</Button>
		{/if}
	{/snippet}
	{#if parts.error}
		<p class="error" role="alert">{t.loadFailed}: {parts.error}</p>
		<div>
			<Button size="sm" busy={parts.loading} onclick={() => void parts.load()}
				>{ru.common.retry}</Button
			>
		</div>
	{:else if parts.parts.length > 0}
		<ul>
			{#each parts.parts as p (p.id)}
				<li>
					<div class="line">
						<span class="text">
							{#if canView && p.item_exists && p.stock_item_id !== null}
								<a href={resolve(stockItemHref(p.stock_item_id))}>{p.name}</a>
							{:else}
								{p.name}
							{/if}
							× {p.quantity} — <span class="num">{formatMoney(p.sum)}</span>
						</span>
						{#if canWrite}
							<Button
								size="sm"
								variant="ghost"
								icon={Undo2}
								aria-label="{t.giveBack}: {p.name}"
								busy={parts.isReturning(p.id)}
								onclick={() => (giving = p)}>{t.giveBack}</Button
							>
						{/if}
					</div>
					<p class="meta">
						{meta(p)}{#if !p.item_exists}<span class="num"> · {t.noItem}</span>{/if}
					</p>
					{#if p.places.length > 0}<p class="meta">{p.places.join(', ')}</p>{/if}
				</li>
			{/each}
		</ul>
		<p class="total num">{t.total} <strong>{formatMoney(parts.total)}</strong></p>
	{:else if !parts.loading}
		<p class="meta">{t.empty}</p>
	{/if}
</Section>

{#if canWrite}
	<StockPickDialog bind:open={picking} {parts} />
{/if}
<ConfirmDialog
	bind:open={() => giving !== null, (v) => !v && (giving = null)}
	title={t.giveBackTitle}
	description={giving === null
		? ''
		: giving.item_exists
			? t.giveBackText(giving.name, giving.quantity)
			: t.giveBackNoItem(giving.name, giving.quantity)}
	confirmLabel={t.giveBackConfirm}
	tone="primary"
	onconfirm={() => (giving ? giveBack(giving) : undefined)}
/>

<style>
	ul {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	li {
		overflow-wrap: anywhere;
	}
	.line {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}
	.text {
		min-width: 0;
	}
	p {
		margin: 0;
	}
	.meta {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.total {
		text-align: end;
	}
	.error {
		color: var(--danger-text);
	}
</style>
