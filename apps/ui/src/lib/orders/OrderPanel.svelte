<!-- The order panel (spec 6.2, 6.12): opens over any screen of the program without darkening it. Edits save
     by themselves; moving to another order or closing waits until they are saved. On a phone it fills the
     screen, the status is a button and the bottom bar calls the client. -->
<script lang="ts">
	import { hasCommentDrafts } from './comment-drafts.svelte.js';
	import { display } from '#lib/design/prefs.svelte.js';
	import Camera from '@lucide/svelte/icons/camera';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Maximize2 from '@lucide/svelte/icons/maximize-2';
	import Phone from '@lucide/svelte/icons/phone';
	import { tick, untrack } from 'svelte';
	import { beforeNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { Client } from '$api/Client';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { errorMessage } from '#lib/api/client.js';
	import { getClient } from '#lib/api/clients.js';
	import { listMasters } from '#lib/api/masters.js';
	import { getOrder } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { clientHref } from '#lib/clients/view.js';
	import { formatRelative } from '#lib/format/date.js';
	import { dialNumber } from '#lib/format/phone.js';
	import { ru } from '#lib/i18n/ru.js';
	import PrintMenu from '#lib/print/PrintMenu.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import SlideOver from '#lib/ui/SlideOver.svelte';
	import StatusPicker from '#lib/ui/StatusPicker.svelte';
	import { closeEditor, OrderActions } from './actions.svelte.js';
	import { createOrderEditor, type OrderEditor } from './editor.svelte.js';
	import { onOrderChanged } from './events.js';
	import { dispute } from './dispute.svelte.js';
	import type { MasterList } from './masters.js';
	import OrderComments from './OrderComments.svelte';
	import OrderDialogs from './OrderDialogs.svelte';
	import OrderFields from './OrderFields.svelte';
	import OrderHistory from './OrderHistory.svelte';
	import OrderParts from './OrderParts.svelte';
	import OrderPhotos from './OrderPhotos.svelte';
	import { createOrderParts, type OrderPartsState } from './parts.svelte.js';
	import { orderPanel } from './panel.svelte.js';
	import { createOrderMedia, limitPhotos, type OrderMedia } from './photos.svelte.js';
	import SaveIndicator from './SaveIndicator.svelte';
	import { deviceOf } from './view.js';

	const t = ru.order;

	/** The order shown, one editor per order; another order gets a new one. */
	let editor = $state.raw<OrderEditor | null>(null);
	let client = $state.raw<Client | null>(null);
	/** The photos and files of the order shown, one set per order. */
	let media = $state.raw<OrderMedia | null>(null);
	/** The parts written off into the order shown, one set per order. */
	let parts = $state.raw<OrderPartsState | null>(null);
	let cameraInput = $state<HTMLInputElement | null>(null);
	let loadError = $state<string | null>(null);
	let dictionaries = $state.raw<{ masters: MasterList; warranty: string[] }>({
		masters: null,
		warranty: []
	});
	let body = $state<HTMLElement | null>(null);
	/** The field that had focus when the panel moved on: the same field of the next order gets it. */
	let refocusId: string | null = null;

	/** Status, «Выдать», «Вернуть в работу», «Принять по гарантии» of the order shown. */
	const orderActions = new OrderActions(() => editor);
	const order = $derived(editor?.order ?? null);
	const status = $derived(orderActions.status);
	const issued = $derived(orderActions.issued);
	const canEdit = $derived(orderActions.canEdit);
	const pointName = $derived(
		order?.service_point_id == null
			? ''
			: (app.points.find((p) => p.id === order.service_point_id)?.name ?? '')
	);
	const shownClient = $derived(client && order && client.id === order.client_id ? client : null);
	const mainPhone = $derived(shownClient?.phones[0]?.phone ?? '');
	const title = $derived(
		order === null
			? t.title(orderPanel.id ?? 0)
			: display.phone
				? t.title(order.id)
				: `${t.title(order.id)} · ${deviceOf(order)}`
	);

	// The masters and the dictionary «Гарантия», asked again at every opening: settings may have changed.
	// Each on its own: one not read leaves the other.
	const isOpen = $derived(orderPanel.id !== null);
	$effect(() => {
		if (!isOpen) return;
		let current = true;
		listMasters().then(
			(masters) => {
				if (current) dictionaries = { ...dictionaries, masters };
			},
			() => {
				// Not read: the chosen master keeps its name (an earlier list stays).
				if (current && dictionaries.masters === null)
					dictionaries = { ...dictionaries, masters: 'failed' };
			}
		);
		listDictionary('warranty').then(
			(warranty) => {
				if (current) dictionaries = { ...dictionaries, warranty: warranty.map((d) => d.value) };
			},
			() => {
				// Without it the field still takes any text.
			}
		);
		return () => {
			current = false;
		};
	});

	let token = 0;
	$effect(() => {
		const id = orderPanel.id;
		untrack(() => void load(id));
	});

	async function load(id: number | null) {
		const mine = ++token;
		if (id === null) {
			swap(null);
			client = null;
			loadError = null;
			refocusId = null;
			return;
		}
		if (editor?.order.id === id) return;
		loadError = null;
		try {
			const o = await getOrder(id);
			if (mine !== token) return;
			swap(createOrderEditor(o));
			if (client?.id !== o.client_id) void loadClient(o.client_id);
			await tick();
			await restoreFocus();
		} catch (e) {
			if (mine !== token) return;
			swap(null);
			loadError = errorMessage(e);
		}
	}

	async function loadClient(id: number) {
		try {
			const c = await getClient(id);
			if (editor?.order.client_id === id) client = c;
		} catch {
			// No right to see clients, or no connection: «Клиент № …» stays.
		}
	}

	function swap(next: OrderEditor | null) {
		const old = editor;
		editor = next;
		media?.detach();
		media = next ? createOrderMedia(next.order.id) : null;
		parts?.detach();
		parts = next ? createOrderParts(next.order.id) : null;
		void parts?.load();
		// The guard saved it; what was typed while the next order was loading is saved now.
		if (old) closeEditor(old);
	}

	async function restoreFocus() {
		const id = refocusId;
		refocusId = null;
		const el = id ? document.getElementById(id) : null;
		if (!(el instanceof HTMLElement) || !body?.contains(el)) return;
		el.focus({ preventScroll: true });
		await tick();
		if (el instanceof HTMLTextAreaElement || el instanceof HTMLInputElement) {
			const end = el.value.length;
			el.setSelectionRange(end, end);
		}
	}

	// Moving on and closing wait until the edits are saved; if they cannot be, the panel stays.
	$effect(() =>
		orderPanel.guard(async () => {
			const ed = editor;
			if (!ed) return true;
			const active = document.activeElement;
			const ok = await ed.leave(() => dispute.ask(ed.order.id));
			if (ok && active instanceof HTMLElement && active.id && body?.contains(active)) {
				refocusId = active.id;
			}
			return ok;
		})
	);

	// Closing or reloading the tab while edits wait or failed gets the browser's own question.
	beforeNavigate((nav) => {
		if (nav.willUnload && (editor?.unsaved || hasCommentDrafts())) nav.cancel();
	});

	// Changes made elsewhere in the program (an undo, the list) reach the open order.
	$effect(() => onOrderChanged((o) => editor?.receive(o)));
	// The program's frame goes (signing out): what is typed is still sent, a failure is told.
	$effect(
		() => () =>
			untrack(() => {
				if (editor) closeEditor(editor);
			})
	);

	/** ⤢ and «все»: the full card; the panel saves and closes first. A click with a modifier opens a tab. */
	async function openCard(e?: MouseEvent) {
		if (e && (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey || e.button !== 0)) return;
		e?.preventDefault();
		const id = order?.id;
		if (id === undefined) return;
		if (!(await orderPanel.close())) return;
		await goto(resolve(`orders/${id}`));
	}

	const plainClick = (e: MouseEvent) =>
		!(e.ctrlKey || e.metaKey || e.shiftKey || e.altKey || e.button !== 0);

	/** «открыть» at the client: the panel saves and closes, then «Клиенты» shows the client. */
	async function openClient(e: MouseEvent) {
		if (!plainClick(e)) return;
		e.preventDefault();
		const id = order?.client_id;
		if (id === undefined) return;
		if (!(await orderPanel.close())) return;
		await goto(resolve(clientHref(id)));
	}

	/**
	 * «← к клиенту» of an order opened from a client's panel: this panel closes, the client's is under it;
	 * from anywhere else the address leads there.
	 */
	async function goBack(e: MouseEvent) {
		const href = orderPanel.back?.href;
		if (!href || !plainClick(e)) return;
		e.preventDefault();
		if (!(await orderPanel.close())) return;
		const target = new URL(href, location.href);
		if (target.pathname + target.search === location.pathname + location.search) return;
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved by whoever opened the panel
		await goto(href);
	}

	// «Выдать»: Ctrl/⌘+Enter from the panel; its act: Ctrl/⌘P.
	$effect(() => orderActions.issueShortcut());
	$effect(() => orderActions.printShortcut());

	const menu = $derived<MenuItem[]>([
		...orderActions.menuItems,
		...(!issued && display.phone && canEdit
			? [{ label: t.issueMenu, onselect: () => orderActions.openIssue() }]
			: []),
		{ label: t.openCard, onselect: () => void openCard() }
	]);
</script>

{#snippet moreMenu(variant: 'ghost' | 'secondary')}
	<DropdownMenu items={menu} label={t.moreActions}>
		{#snippet trigger(props)}
			<IconButton {...props} label={t.moreActions} icon={Ellipsis} {variant} />
		{/snippet}
	</DropdownMenu>
{/snippet}

{#snippet printMenu(o: NonNullable<typeof order>, compact: boolean)}
	<PrintMenu
		{issued}
		wasIssued={o.issued_at !== null}
		{compact}
		variant={compact ? 'ghost' : 'secondary'}
		onprint={(kind) => void orderActions.print(kind)}
		onpreview={(kind, kinds) => void orderActions.preview(kind, kinds)}
	/>
{/snippet}

{#snippet statusPicker()}
	<StatusPicker
		statuses={orderActions.pickable}
		value={orderActions.statusValue}
		disabled={issued || !canEdit || !editor}
		onchange={(id, previous) => void orderActions.pickStatus(id, previous)}
	/>
{/snippet}

<SlideOver
	bind:open={() => orderPanel.id !== null, (v) => !v && void orderPanel.close()}
	{title}
	onprev={orderPanel.hasNext(-1) ? () => orderPanel.next(-1) : undefined}
	onnext={orderPanel.hasNext(1) ? () => orderPanel.next(1) : undefined}
>
	{#snippet actions()}
		{#if display.phone}
			{#if order}{@render printMenu(order, true)}{/if}
			{@render moreMenu('ghost')}
		{:else if order}
			<IconButton label={t.openCard} icon={Maximize2} onclick={() => void openCard()} />
		{/if}
	{/snippet}
	{#snippet header()}
		{#if orderPanel.back}
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved by whoever opened the panel -->
			<a class="back" href={orderPanel.back.href} onclick={goBack}>{orderPanel.back.label}</a>
		{/if}
		{#if order}
			<p class="meta num">
				<span>{t.received(formatRelative(order.received_at, serverClock.now()))}</span>
				{#if pointName}<span aria-hidden="true">·</span><span>{pointName}</span>{/if}
				{#if editor}<SaveIndicator {editor} />{/if}
			</p>
			{#if !display.phone}{@render statusPicker()}{/if}
		{/if}
	{/snippet}

	<div class="content" bind:this={body}>
		{#if editor}
			{@const ed = editor}
			{#key ed}
				<OrderFields
					order={ed.order}
					autosave={ed.autosave}
					client={shownClient}
					{dictionaries}
					readonly={!canEdit}
					idPrefix="order-panel"
					partsTotal={parts?.total ?? 0}
					conflicts={ed.conflicts}
					flashed={ed.flashed}
					onkeep={(f) => ed.keep(f)}
					ontake={(f) => ed.take(f)}
					onflashend={(f) => ed.endFlash(f)}
					onopenclient={openClient}
				/>
				<OrderComments
					orderId={ed.order.id}
					canPost={canEdit && !ed.order.deleted_at && orderPanel.id === ed.order.id}
				/>
				{#if parts}<OrderParts {parts} orderDeleted={ed.order.deleted_at !== null} />{/if}
				{#if media}<OrderPhotos {media} readonly={!canEdit} />{/if}
				<OrderHistory orderId={ed.order.id} version={ed.order.version} onall={openCard} />
			{/key}
		{:else if loadError}
			<Callout tone="danger" title={t.loadFailed}>
				{loadError}
				{#snippet actions()}
					<Button size="sm" onclick={() => void load(orderPanel.id)}>{ru.common.retry}</Button>
				{/snippet}
			</Callout>
		{:else}
			<div class="loading" aria-busy="true">
				<Skeleton height="1.25rem" width="60%" />
				<Skeleton height="4rem" />
				<Skeleton height="4rem" />
				<Skeleton height="2.5rem" width="80%" />
			</div>
		{/if}
	</div>

	{#snippet footer()}
		{#if display.phone}
			{#if mainPhone}
				<Button href="tel:{dialNumber(mainPhone)}" icon={Phone}>{t.call}</Button>
			{/if}
			{#if media && canEdit}
				<Button icon={Camera} onclick={() => cameraInput?.click()}>{ru.media.photoButton}</Button>
				<input
					bind:this={cameraInput}
					type="file"
					hidden
					accept="image/*"
					capture="environment"
					onchange={(e) => {
						if (media) void media.add(limitPhotos(Array.from(e.currentTarget.files ?? [])));
						e.currentTarget.value = '';
					}}
				/>
			{/if}
			<span class="grow"></span>
			{@render statusPicker()}
		{:else}
			{@render moreMenu('secondary')}
			{#if order}{@render printMenu(order, false)}{/if}
			<span class="grow"></span>
			{#if order && !issued}
				<Button
					variant={status?.kind === 'ready' ? 'primary' : 'secondary'}
					shortcut="Mod+Enter"
					disabled={!canEdit}
					onclick={() => orderActions.openIssue()}>{t.issue}</Button
				>
			{/if}
		{/if}
	{/snippet}
</SlideOver>

<OrderDialogs actions={orderActions} {dictionaries} />

<style>
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1-5);
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.back {
		align-self: flex-start;
		color: var(--accent);
		font-size: var(--text-sm);
		text-decoration: none;
	}
	.back:hover {
		text-decoration: underline;
	}
	.loading {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.grow {
		flex: 1;
	}
</style>
