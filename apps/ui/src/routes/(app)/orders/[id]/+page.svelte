<!-- The full order card (spec 6.3): the fields on the left, statuses, warranty cases and the whole
     history on the right; narrower than 1024px the two are tabs. Edits save by themselves as in the
     panel; the status, «Выдать» and ⋯ are the panel's too. -->
<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import { untrack } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { afterNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Client } from '$api/Client';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { getClient } from '#lib/api/clients.js';
	import { getOrder } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { pushRecent } from '#lib/app/recent.js';
	import { CARD_COLUMNS_QUERY } from '#lib/design/breakpoints.js';
	import { formatDate, formatRelative } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import { closeEditor, OrderActions } from '#lib/orders/actions.svelte.js';
	import { createCardHistory } from '#lib/orders/card-history.svelte.js';
	import { createOrderEditor, type OrderEditor } from '#lib/orders/editor.svelte.js';
	import { onOrderChanged } from '#lib/orders/events.js';
	import { historyItems, statusTimeline, warrantyCases } from '#lib/orders/history.js';
	import OrderDialogs from '#lib/orders/OrderDialogs.svelte';
	import OrderFields from '#lib/orders/OrderFields.svelte';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import SaveIndicator from '#lib/orders/SaveIndicator.svelte';
	import { deviceOf } from '#lib/orders/view.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import DropdownMenu from '#lib/ui/DropdownMenu.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import StatusPicker from '#lib/ui/StatusPicker.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';
	import Timeline from '#lib/ui/Timeline.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.order;
	const c = ru.card;
	const columns = new MediaQuery(CARD_COLUMNS_QUERY);

	const param = $derived(page.params.id ?? '');
	const id = $derived(/^\d+$/.test(param) ? Number(param) : null);

	let editor = $state.raw<OrderEditor | null>(null);
	let client = $state.raw<Client | null>(null);
	let loadError = $state<string | null>(null);
	let notFound = $state(false);
	let dictionaries = $state.raw<{
		master: string[];
		warranty: string[];
		device_type: string[];
		brand: string[];
		color: string[];
	}>({ master: [], warranty: [], device_type: [], brand: [], color: [] });

	const orderActions = new OrderActions(() => editor);
	const order = $derived(editor?.order ?? null);
	const issued = $derived(orderActions.issued);
	const canEdit = $derived(orderActions.canEdit);
	const shownClient = $derived(client && order && client.id === order.client_id ? client : null);
	const pointName = $derived(
		order?.service_point_id == null
			? ''
			: (app.points.find((p) => p.id === order.service_point_id)?.name ?? '')
	);

	// The dictionaries of the fields, once per opening of the card: settings may have changed.
	$effect(() => {
		const kinds = ['master', 'warranty', 'device_type', 'brand', 'color'] as const;
		let current = true;
		Promise.all(kinds.map((k) => listDictionary(k))).then(
			(lists) => {
				if (!current) return;
				dictionaries = Object.fromEntries(
					kinds.map((k, i) => [k, (lists[i] ?? []).map((d) => d.value)])
				) as typeof dictionaries;
			},
			() => {
				// Without them the fields still take any text.
			}
		);
		return () => {
			current = false;
		};
	});

	let token = 0;
	$effect(() => {
		const n = id;
		untrack(() => void load(n));
	});
	// Leaving the card: an order still loading is not shown, the one shown saves what is typed.
	$effect(
		() => () =>
			untrack(() => {
				token += 1;
				swap(null);
			})
	);

	async function load(n: number | null) {
		const mine = ++token;
		if (editor && editor.order.id === n) return;
		swap(null);
		loadError = null;
		notFound = false;
		if (n === null) {
			notFound = true;
			return;
		}
		try {
			const o = await getOrder(n);
			if (mine !== token) return;
			swap(createOrderEditor(o));
			pushRecent(o.id);
			if (client?.id !== o.client_id) void loadClient(o.client_id);
		} catch (e) {
			if (mine !== token) return;
			if (e instanceof ApiError && e.status === 404) notFound = true;
			else loadError = errorMessage(e);
		}
	}

	async function loadClient(clientId: number) {
		try {
			const found = await getClient(clientId);
			if (editor?.order.client_id === clientId) client = found;
		} catch {
			// No right to see clients, or no connection: «Клиент № …» stays.
		}
	}

	function swap(next: OrderEditor | null) {
		const old = editor;
		editor = next;
		// Another order or the screen left: what is typed is still saved.
		if (old) closeEditor(old);
	}

	// Changes made elsewhere in the program (an undo, a bulk action, the panel) reach the card.
	$effect(() => onOrderChanged((o) => editor?.receive(o)));

	// The history follows every new version: a status picked here is in «Статусы и сроки» at once.
	const journal = createCardHistory();
	const version = $derived(order?.version);
	$effect(() => {
		const n = order?.id;
		void version;
		if (n !== undefined) untrack(() => void journal.refresh(n));
	});
	async function more() {
		try {
			await journal.more();
		} catch {
			toast.show({ message: t.historyFailed, tone: 'danger' });
		}
	}
	const timeline = $derived(statusTimeline(journal.newest));
	const cases = $derived(warrantyCases(journal.newest));

	// «←»: back where the card was opened from; opened by its address — to the list.
	let cameFromApp = false;
	afterNavigate(({ from }) => {
		cameFromApp = from !== null;
	});
	function back() {
		if (cameFromApp) history.back();
		else void goto(resolve('orders'));
	}

	// «Выдать» on Ctrl/⌘+Enter; the order panel over the card has its own.
	$effect(() => orderActions.issueShortcut(() => orderPanel.id === null));

	let tab = $state('order');
	const TABS = [
		{ value: 'order', label: c.order },
		{ value: 'history', label: c.history }
	];
	const docTitle = $derived(order ? `${t.title(order.id)} · ${deviceOf(order)}` : t.title(id ?? 0));
</script>

<svelte:head><title>{docTitle}</title></svelte:head>

{#snippet fields(ed: OrderEditor)}
	{#key ed}
		<OrderFields
			order={ed.order}
			autosave={ed.autosave}
			client={shownClient}
			{dictionaries}
			readonly={!canEdit}
			editDevice
			idPrefix="order-card"
			conflicts={ed.conflicts}
			flashed={ed.flashed}
			onkeep={(f) => ed.keep(f)}
			ontake={(f) => ed.take(f)}
			onflashend={(f) => ed.endFlash(f)}
		/>
	{/key}
{/snippet}

{#snippet side()}
	<div class="side">
		<Section title={c.statuses} region>
			{#if journal.failed}
				<p class="muted">{t.historyFailed}</p>
			{:else}
				<Timeline items={timeline} now={serverClock.now()} emptyText={t.historyEmpty} />
			{/if}
		</Section>
		<Section title={c.warrantyCases} region>
			{#if cases.length === 0}
				<p class="muted">{c.noWarrantyCases}</p>
			{:else}
				<ul class="cases">
					{#each cases as w (w.id)}
						<li>
							<time class="num muted" datetime={w.at}>{formatDate(w.at)}</time>
							<span>{w.complaint}</span>
						</li>
					{/each}
				</ul>
			{/if}
		</Section>
		<Section title={c.changes} region>
			{#if journal.failed}
				<p class="muted">{t.historyFailed}</p>
			{:else}
				<Timeline
					items={historyItems(journal.entries)}
					now={serverClock.now()}
					emptyText={t.historyEmpty}
				/>
				{#if journal.hasMore}
					<div>
						<Button size="sm" busy={journal.loadingMore} onclick={() => void more()}
							>{c.showMore}</Button
						>
					</div>
				{/if}
			{/if}
		</Section>
	</div>
{/snippet}

<div class="card">
	<header class="head">
		<div class="title">
			<IconButton label={ru.common.back} icon={ArrowLeft} onclick={back} />
			<h1 class="num">{t.title(order?.id ?? id ?? 0)}</h1>
			{#if order}<span class="device">{deviceOf(order)}</span>{/if}
			<span class="grow"></span>
			{#if order && !issued}
				<Button
					variant={orderActions.status?.kind === 'ready' ? 'primary' : 'secondary'}
					shortcut="Mod+Enter"
					disabled={!canEdit}
					onclick={() => orderActions.openIssue()}>{t.issue}</Button
				>
			{/if}
			{#if orderActions.menuItems.length > 0}
				<DropdownMenu items={orderActions.menuItems} label={t.moreActions}>
					{#snippet trigger(props)}
						<IconButton {...props} label={t.moreActions} icon={Ellipsis} variant="secondary" />
					{/snippet}
				</DropdownMenu>
			{/if}
		</div>
		{#if order && editor}
			<p class="meta num">
				<span>{t.received(formatRelative(order.received_at, serverClock.now()))}</span>
				{#if pointName}<span aria-hidden="true">·</span><span>{pointName}</span>{/if}
				<SaveIndicator {editor} />
			</p>
			<StatusPicker
				statuses={orderActions.pickable}
				value={orderActions.statusValue}
				disabled={issued || !canEdit}
				onchange={(next, previous) => void orderActions.pickStatus(next, previous)}
			/>
		{/if}
	</header>

	{#if editor}
		{#if columns.current}
			<div class="columns">
				<div class="fields">{@render fields(editor)}</div>
				<aside class="aside" aria-label={c.history}>{@render side()}</aside>
			</div>
		{:else}
			{@const ed = editor}
			<Tabs bind:value={tab} items={TABS}>
				{#snippet content(v)}
					{#if v === 'order'}{@render fields(ed)}{:else}{@render side()}{/if}
				{/snippet}
			</Tabs>
		{/if}
	{:else if notFound}
		<EmptyState title={c.notFound(param)} description={c.notFoundHint}>
			{#snippet actions()}
				<Button size="sm" href={resolve('orders')}>{ru.app.toOrders}</Button>
			{/snippet}
		</EmptyState>
	{:else if loadError}
		<Callout tone="danger" title={t.loadFailed}>
			{loadError}
			{#snippet actions()}
				<Button size="sm" onclick={() => void load(id)}>{ru.common.retry}</Button>
			{/snippet}
		</Callout>
	{:else}
		<div class="loading" aria-busy="true">
			<Skeleton height="1.25rem" width="40%" />
			<Skeleton height="4rem" />
			<Skeleton height="4rem" />
			<Skeleton height="2.5rem" width="70%" />
		</div>
	{/if}
</div>

<OrderDialogs actions={orderActions} {dictionaries} />

<style>
	.card {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
		max-width: 80rem;
		margin: 0 auto;
		padding: var(--space-4) var(--space-6) var(--space-10);
	}
	.head {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.title {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}
	h1 {
		flex: none;
		margin: 0;
		font-size: var(--text-2xl);
		font-weight: 600;
	}
	.device {
		min-width: 0;
		overflow: hidden;
		color: var(--text-2);
		font-size: var(--text-lg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1-5);
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.grow {
		flex: 1;
	}
	.columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(18rem, 26rem);
		align-items: start;
		gap: var(--space-8);
	}
	.side {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}
	.cases {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.cases li {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
		overflow-wrap: anywhere;
	}
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.loading {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	@media (max-width: 599px) {
		.card {
			padding: var(--space-3) var(--space-3) var(--space-8);
		}
	}
</style>
