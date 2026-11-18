<!-- The order panel (spec 6.2, 6.12): opens over any screen of the program without darkening it. Edits save
     by themselves; moving to another order or closing waits until they are saved. On a phone it fills the
     screen, the status is a button and the bottom bar calls the client. -->
<script lang="ts">
	import Camera from '@lucide/svelte/icons/camera';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Maximize2 from '@lucide/svelte/icons/maximize-2';
	import Phone from '@lucide/svelte/icons/phone';
	import { tick, untrack } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { beforeNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { Client } from '$api/Client';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { errorMessage } from '#lib/api/client.js';
	import { getClient } from '#lib/api/clients.js';
	import { getOrder } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { formatRelative } from '#lib/format/date.js';
	import { dialNumber } from '#lib/format/phone.js';
	import { ru } from '#lib/i18n/ru.js';
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
	import OrderDialogs from './OrderDialogs.svelte';
	import OrderFields from './OrderFields.svelte';
	import OrderHistory from './OrderHistory.svelte';
	import OrderPhotos from './OrderPhotos.svelte';
	import { orderPanel } from './panel.svelte.js';
	import { createOrderMedia, limitPhotos, type OrderMedia } from './photos.svelte.js';
	import SaveIndicator from './SaveIndicator.svelte';
	import { deviceOf } from './view.js';

	const t = ru.order;
	const phone = new MediaQuery(PHONE_QUERY);

	/** The order shown, one editor per order; another order gets a new one. */
	let editor = $state.raw<OrderEditor | null>(null);
	let client = $state.raw<Client | null>(null);
	/** The photos and files of the order shown, one set per order. */
	let media = $state.raw<OrderMedia | null>(null);
	let cameraInput = $state<HTMLInputElement | null>(null);
	let loadError = $state<string | null>(null);
	let dictionaries = $state.raw<{ master: string[]; warranty: string[] }>({
		master: [],
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
			: phone.current
				? t.title(order.id)
				: `${t.title(order.id)} · ${deviceOf(order)}`
	);

	// Dictionaries of «Мастер» and «Гарантия», asked again at every opening: settings may have changed.
	const isOpen = $derived(orderPanel.id !== null);
	$effect(() => {
		if (!isOpen) return;
		let current = true;
		Promise.all([listDictionary('master'), listDictionary('warranty')]).then(
			([master, warranty]) => {
				if (!current) return;
				dictionaries = {
					master: master.map((d) => d.value),
					warranty: warranty.map((d) => d.value)
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
			const ok = await ed.flush();
			if (ok && active instanceof HTMLElement && active.id && body?.contains(active)) {
				refocusId = active.id;
			}
			return ok;
		})
	);

	// Closing or reloading the tab while edits wait or failed gets the browser's own question.
	beforeNavigate((nav) => {
		if (nav.willUnload && editor?.unsaved) nav.cancel();
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

	// «Выдать»: Ctrl/⌘+Enter from the panel.
	$effect(() => orderActions.issueShortcut());

	const menu = $derived<MenuItem[]>([
		...orderActions.menuItems,
		...(!issued && phone.current && canEdit
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
		{#if phone.current}
			{@render moreMenu('ghost')}
		{:else if order}
			<IconButton label={t.openCard} icon={Maximize2} onclick={() => void openCard()} />
		{/if}
	{/snippet}
	{#snippet header()}
		{#if order}
			<p class="meta num">
				<span>{t.received(formatRelative(order.received_at, serverClock.now()))}</span>
				{#if pointName}<span aria-hidden="true">·</span><span>{pointName}</span>{/if}
				{#if editor}<SaveIndicator {editor} />{/if}
			</p>
			{#if !phone.current}{@render statusPicker()}{/if}
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
					conflicts={ed.conflicts}
					flashed={ed.flashed}
					onkeep={(f) => ed.keep(f)}
					ontake={(f) => ed.take(f)}
					onflashend={(f) => ed.endFlash(f)}
				/>
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
		{#if phone.current}
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
		gap: var(--space-5);
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
