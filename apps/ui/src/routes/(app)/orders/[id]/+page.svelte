<!-- The full order card (spec 6.3): the fields on the left, statuses, warranty cases and the whole
     history on the right; narrower than 1024px the two are tabs. Edits save by themselves as in the
     panel; the status, «Выдать» and ⋯ are the panel's too. -->
<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import { untrack } from 'svelte';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Client } from '$api/Client';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { getClient } from '#lib/api/clients.js';
	import { listMasters } from '#lib/api/masters.js';
	import { getOrder } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { isSignIn } from '#lib/app/login.js';
	import { pushRecent } from '#lib/app/recent.js';
	import { formatDate, formatRelative } from '#lib/format/date.js';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';
	import { closeEditor, OrderActions, unsavedText } from '#lib/orders/actions.svelte.js';
	import { createCardHistory } from '#lib/orders/card-history.svelte.js';
	import { createOrderEditor, type OrderEditor } from '#lib/orders/editor.svelte.js';
	import { dispute } from '#lib/orders/dispute.svelte.js';
	import { onOrderChanged } from '#lib/orders/events.js';
	import { historyItems, statusTimeline, warrantyCases } from '#lib/orders/history.js';
	import type { MasterList } from '#lib/orders/masters.js';
	import OrderDialogs from '#lib/orders/OrderDialogs.svelte';
	import OrderFields from '#lib/orders/OrderFields.svelte';
	import OrderFiles from '#lib/orders/OrderFiles.svelte';
	import OrderPhotos from '#lib/orders/OrderPhotos.svelte';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import { createOrderMedia, type OrderMedia } from '#lib/orders/photos.svelte.js';
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
	import TabPane from '#lib/ui/TabPane.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';
	import Timeline from '#lib/ui/Timeline.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.order;
	const c = ru.card;

	const param = $derived(page.params.id ?? '');
	const id = $derived(/^\d+$/.test(param) ? Number(param) : null);

	let editor = $state.raw<OrderEditor | null>(null);
	let client = $state.raw<Client | null>(null);
	/** The photos and files of the order shown. */
	let media = $state.raw<OrderMedia | null>(null);
	let loadError = $state<string | null>(null);
	let notFound = $state(false);
	let dictionaries = $state.raw<{
		masters: MasterList;
		warranty: string[];
		device_type: string[];
		brand: string[];
		color: string[];
	}>({ masters: null, warranty: [], device_type: [], brand: [], color: [] });

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

	// The dictionaries of the fields, once per opening of the card: settings may have changed. The masters
	// on their own: dictionaries not read leave them, and the other way round.
	$effect(() => {
		const kinds = ['warranty', 'device_type', 'brand', 'color'] as const;
		let current = true;
		listMasters().then(
			(masters) => {
				if (current) dictionaries = { ...dictionaries, masters };
			},
			() => {
				// Not read: the chosen master keeps its name.
				if (current) dictionaries = { ...dictionaries, masters: 'failed' };
			}
		);
		Promise.all(kinds.map((k) => listDictionary(k))).then(
			(lists) => {
				if (!current) return;
				dictionaries = {
					...dictionaries,
					...(Object.fromEntries(
						kinds.map((k, i) => [k, (lists[i] ?? []).map((d) => d.value)])
					) as Omit<typeof dictionaries, 'masters'>)
				};
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
		media?.detach();
		media = next ? createOrderMedia(next.order.id) : null;
		// Another order or the screen left: what is typed is still saved.
		if (old) closeEditor(old);
	}

	// Changes made elsewhere in the program (an undo, a bulk action, the panel) reach the card.
	$effect(() => onOrderChanged((o) => editor?.receive(o)));
	// The scanner or Ctrl K asking for this very order shows the card, not a panel over it.
	$effect(() => {
		const n = id;
		if (n !== null) return untrack(() => orderPanel.cardShows(n));
	});

	// The history follows every new version: a status picked here is in «Статусы и сроки» at once.
	const journal = createCardHistory();
	const version = $derived(order?.version);
	$effect(() => {
		const n = order?.id;
		void version;
		if (n !== undefined) untrack(() => void journal.refresh(n));
	});
	/** «Повторить» of the timeline: the history of the order shown; none while there is no order. */
	function retryTimeline() {
		if (order) void journal.refresh(order.id);
	}
	const timeline = $derived(statusTimeline(journal.timeline));
	const cases = $derived(warrantyCases(journal.timeline));

	/**
	 * Before the card is left what is typed is saved. When it cannot be (no connection, a conflict to
	 * decide) the user stays here, the toast says why; false: stay.
	 */
	async function saved(ed: OrderEditor): Promise<boolean> {
		if (!ed.unsaved) return true;
		const ok = await ed.leave(() => dispute.ask(ed.order.id));
		// A dispute left undecided was asked about in its own dialog.
		if (!ok && editor === ed && ed.conflicts.size === 0)
			toast.show({ message: unsavedText(ed), tone: 'danger' });
		return ok;
	}

	// «←»: back where the card was opened from; opened by its address — to the list.
	let cameFromApp = false;
	afterNavigate(({ from }) => {
		cameFromApp = from !== null;
	});
	let goingBack = false;
	async function back() {
		if (goingBack) return;
		goingBack = true;
		try {
			const ed = editor;
			if (ed && !(await saved(ed))) return;
			if (cameFromApp) history.back();
			else await goto(resolve('orders'));
		} finally {
			goingBack = false;
		}
	}

	// Any other way out (a link, Ctrl K, the browser's «Назад») waits for the save too and then goes on.
	// Closing or reloading the tab with unsaved changes gets the browser's own question. The sign-in page
	// (session expired, signed out, new password) is not held: without a session nothing can be saved,
	// and leaving the card tells what was lost.
	let resume: (() => void) | null = null;
	beforeNavigate((nav) => {
		const ed = editor;
		if (!ed?.unsaved || (nav.to && isSignIn(nav.to.url))) return;
		nav.cancel();
		if (nav.willUnload || !nav.to) return;
		const { url } = nav.to;
		const delta = nav.type === 'popstate' ? nav.delta : 0;
		const waiting = resume !== null;
		resume =
			delta !== 0
				? () => history.go(delta)
				: // eslint-disable-next-line svelte/no-navigation-without-resolve -- where SvelteKit was going
					() => void goto(url);
		if (waiting) return;
		void saved(ed).then((ok) => {
			const go = resume;
			resume = null;
			if (ok && editor === ed) go?.();
		});
	});

	// «Выдать» on Ctrl/⌘+Enter; the order panel over the card has its own.
	$effect(() => orderActions.issueShortcut(() => orderPanel.id === null));

	let tab = $state('order');
	const TABS = [
		{ value: 'order', label: c.order },
		{ value: 'history', label: c.history },
		{ value: 'files', label: c.files }
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

{#snippet cut()}
	{#if journal.timelineCut !== null}
		<p class="muted">{c.timelineCut(countLabel(journal.timelineCut, c.entryForms))}</p>
	{/if}
{/snippet}

{#snippet side()}
	<div class="side">
		<TabPane value="history" bare={display.cardColumns}>
			<div class="group">
				<div class="s1">
					<Section title={c.statuses} region>
						{#if journal.failed}
							<p class="muted">{t.historyFailed}</p>
						{:else}
							<Timeline items={timeline} now={serverClock.now()} emptyText={t.historyEmpty} />
							{#if journal.timelineFailed}
								<p class="muted">{c.timelineFailed}</p>
								<div>
									<Button size="sm" onclick={retryTimeline}>{ru.common.retry}</Button>
								</div>
							{/if}
							{@render cut()}
						{/if}
					</Section>
				</div>
				<div class="s4">
					<Section title={c.warrantyCases} help={ru.help.order.warrantyCases} region>
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
						{@render cut()}
					</Section>
				</div>
				<div class="s5">
					<Section title={c.changes} region>
						{#if journal.failed}
							<p class="muted">{t.historyFailed}</p>
						{:else}
							<Timeline
								items={historyItems(journal.entries)}
								now={serverClock.now()}
								emptyText={t.historyEmpty}
							/>
							{#if journal.moreFailed}
								<p class="muted">{c.moreFailed}</p>
							{/if}
							{#if journal.hasMore}
								<div>
									<Button size="sm" busy={journal.loadingMore} onclick={() => void journal.more()}
										>{journal.moreFailed ? ru.common.retry : c.showMore}</Button
									>
								</div>
							{/if}
						{/if}
					</Section>
				</div>
			</div>
		</TabPane>
		<TabPane value="files" bare={display.cardColumns}>
			<div class="group">
				{#if media}
					<div class="s2">
						<Section title={c.photos} region>
							<OrderPhotos {media} readonly={!canEdit} />
						</Section>
					</div>
					<div class="s3">
						<Section title={c.files} region>
							<OrderFiles {media} readonly={!canEdit} />
						</Section>
					</div>
				{/if}
			</div>
		</TabPane>
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
		<!-- One markup for any width: the card is not rebuilt (nor the focus lost) when the window
		     crosses 1024px; only the tab list and the tab semantics of the panes change. -->
		<Tabs bind:value={tab} items={TABS} hideList={display.cardColumns}>
			<div class="layout" class:columns={display.cardColumns}>
				<TabPane value="order" bare={display.cardColumns}>
					<div class="fields">{@render fields(editor)}</div>
				</TabPane>
				<div
					class="aside"
					role={display.cardColumns ? 'complementary' : undefined}
					aria-label={display.cardColumns ? c.history : undefined}
				>
					{@render side()}
				</div>
			</div>
		</Tabs>
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
	.layout.columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(18rem, 26rem);
		align-items: start;
		gap: var(--space-8);
	}
	.side,
	.group {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}
	/* Wide: the groups of the tabs melt into one column in the order of the sections. */
	.columns .group {
		display: contents;
	}
	.s1 {
		order: 1;
	}
	.s2 {
		order: 2;
	}
	.s3 {
		order: 3;
	}
	.s4 {
		order: 4;
	}
	.s5 {
		order: 5;
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
	:global([data-width='xs']) .card {
		padding: var(--space-3) var(--space-3) var(--space-8);
	}
</style>
