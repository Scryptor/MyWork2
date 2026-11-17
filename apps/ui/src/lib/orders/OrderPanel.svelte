<!-- The order panel (spec 6.2, 6.12): opens over any screen of the program without darkening it. Edits save
     by themselves; moving to another order or closing waits until they are saved. On a phone it fills the
     screen, the status is a button and the bottom bar calls the client. -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Maximize2 from '@lucide/svelte/icons/maximize-2';
	import Phone from '@lucide/svelte/icons/phone';
	import { tick, untrack } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { goto } from '$app/navigation';
	import type { Client } from '$api/Client';
	import type { OrderPatch } from '$api/OrderPatch';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { errorMessage } from '#lib/api/client.js';
	import { getClient } from '#lib/api/clients.js';
	import { changeStatus, getOrder, issueOrder, updateOrder } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { undo } from '#lib/app/undo.svelte.js';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
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
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import {
		createOrderEditor,
		revertIssue,
		revertStatus,
		type OrderEditor
	} from './editor.svelte.js';
	import { onOrderChanged } from './events.js';
	import IssueDialog, { type IssueValues } from './IssueDialog.svelte';
	import OrderFields from './OrderFields.svelte';
	import OrderHistory from './OrderHistory.svelte';
	import { orderPanel } from './panel.svelte.js';
	import ReturnToWorkDialog from './ReturnToWorkDialog.svelte';
	import { deviceOf } from './view.js';

	const t = ru.order;
	const phone = new MediaQuery(PHONE_QUERY);

	/** The order shown, one editor per order; another order gets a new one. */
	let editor = $state.raw<OrderEditor | null>(null);
	let client = $state.raw<Client | null>(null);
	let loadError = $state<string | null>(null);
	let dictionaries = $state.raw<{ master: string[]; warranty: string[] }>({
		master: [],
		warranty: []
	});
	let body = $state<HTMLElement | null>(null);
	/** The field that had focus when the panel moved on: the same field of the next order gets it. */
	let refocusId: string | null = null;

	const order = $derived(editor?.order ?? null);
	const status = $derived(order ? app.statusById(order.status_id) : undefined);
	const issued = $derived(status?.kind === 'issued');
	const canEdit = $derived(app.can('orders_edit'));
	const pickable = $derived(app.statuses.filter((s) => !s.archived || s.id === order?.status_id));
	const backToWork = $derived(app.statuses.filter((s) => !s.archived && s.kind !== 'issued'));
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

	/** A status picked a moment ago, shown until the server answers. */
	let picked = $state.raw<{ ed: OrderEditor; id: number } | null>(null);
	const statusValue = $derived(
		picked && picked.ed === editor ? picked.id : (order?.status_id ?? 0)
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
		if (!old) return;
		// The guard saved it; what was typed while the next order was loading is saved now.
		void old
			.flush()
			.then((ok) => {
				const e = old.autosave.error;
				if (!ok && e) toast.show({ message: t.unsaved(old.order.id, e.message), tone: 'danger' });
			})
			.finally(() => old.dispose());
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

	// Changes made elsewhere in the program (an undo, the list) reach the open order.
	$effect(() => onOrderChanged((o) => editor?.receive(o)));
	$effect(() => () => untrack(() => editor?.dispose()));

	async function pickStatus(id: number, previous: number) {
		const ed = editor;
		const s = app.statusById(id);
		if (!ed || !s) return;
		// An issued status is not set here: «Выдать» checks what is required first (spec 6.4).
		if (s.kind === 'issued') {
			openIssue(id);
			return;
		}
		picked = { ed, id };
		try {
			const o = await ed.run((v) => changeStatus(ed.order.id, v, { status_id: id }));
			undo.push(t.statusChanged(o.id, s.name), () => undoStatus(o.id, previous));
		} catch (e) {
			toast.show({ message: t.statusFailed(errorMessage(e)), tone: 'danger' });
		} finally {
			if (picked?.ed === ed) picked = null;
		}
	}

	/** «Отменить»: through the open order's editor when it is shown, else on the newest version. */
	async function undoStatus(id: number, statusId: number) {
		const ed = editor;
		if (ed && ed.order.id === id) {
			await ed.run((v) => changeStatus(id, v, { status_id: statusId }));
		} else {
			await revertStatus(id, statusId);
		}
	}

	let issueOpen = $state(false);
	/** The issued status picked in the panel; null: the first issued one (`issueOrder`). */
	let issueTo = $state<number | null>(null);
	function openIssue(statusId: number | null = null) {
		if (!editor || issued || !canEdit) return;
		issueTo = statusId;
		issueOpen = true;
	}

	async function issue(values: IssueValues) {
		const ed = editor;
		if (!ed) return;
		const id = ed.order.id;
		const before = { status: ed.order.status_id, issuedAt: ed.order.issued_at };
		// What was typed in the panel goes first; then what the dialog changed; then the issue.
		await ed.autosave.flush().catch(() => {});
		const patch: OrderPatch = {};
		if (values.work_done !== ed.order.work_done) patch.work_done = values.work_done;
		if (values.master !== ed.order.master) patch.master = values.master;
		if (values.final_price !== ed.order.final_price) patch.final_price = values.final_price;
		if (values.discount !== ed.order.discount) patch.discount = values.discount;
		if (values.warranty !== ed.order.warranty) patch.warranty = values.warranty;
		if (Object.keys(patch).length > 0) await ed.run((v) => updateOrder(id, v, patch));
		const target = issueTo;
		const o = await ed.run((v) =>
			target === null ? issueOrder(id, v) : changeStatus(id, v, { status_id: target })
		);
		issueOpen = false;
		// Toasts wait for the dialog to go.
		await tick();
		const clearIssuedAt = before.issuedAt === null && app.can('order_dates_edit');
		undo.push(t.statusChanged(id, app.statusById(o.status_id)?.name ?? ''), () =>
			undoIssue(id, before.status, clearIssuedAt)
		);
	}

	async function undoIssue(id: number, statusId: number, clearIssuedAt: boolean) {
		const ed = editor;
		if (ed && ed.order.id === id) {
			const o = await ed.run((v) => changeStatus(id, v, { status_id: statusId }));
			if (clearIssuedAt && o.issued_at !== null) {
				await ed.run((v) => updateOrder(id, v, { issued_at: null }));
			}
		} else {
			await revertIssue(id, { statusId, clearIssuedAt });
		}
	}

	let returnOpen = $state(false);
	async function returnToWork(statusId: number) {
		const ed = editor;
		if (!ed) return;
		const id = ed.order.id;
		const issuedStatus = ed.order.status_id;
		await ed.run((v) => changeStatus(id, v, { status_id: statusId }));
		returnOpen = false;
		await tick();
		undo.push(t.statusChanged(id, app.statusById(statusId)?.name ?? ''), () =>
			undoStatus(id, issuedStatus)
		);
	}

	/** ⤢ and «все»: the full card; the panel saves and closes first. A click with a modifier opens a tab. */
	async function openCard(e?: MouseEvent) {
		if (e && (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey || e.button !== 0)) return;
		e?.preventDefault();
		const id = order?.id;
		if (id === undefined) return;
		if (!(await orderPanel.close())) return;
		// The full card comes with task 9 (route /orders/[id]); then this becomes resolve().
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		await goto(`/orders/${id}`);
	}

	// «Выдать»: Ctrl/⌘+Enter from the panel, fields included, not over a modal (it opens one).
	$effect(() => {
		if (!editor || issued || !canEdit) return;
		return registerHotkey('Mod+Enter', () => openIssue(), { allowInFields: true });
	});

	const menu = $derived<MenuItem[]>([
		...(issued
			? [{ label: t.returnToWork, disabled: !canEdit, onselect: () => (returnOpen = true) }]
			: phone.current && canEdit
				? [{ label: t.issueMenu, onselect: () => openIssue() }]
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
		statuses={pickable}
		value={statusValue}
		disabled={issued || !canEdit || !editor}
		onchange={(id, previous) => void pickStatus(id, previous)}
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
				{#if editor}
					{@const s = editor.saveState}
					<span class="save" aria-live="polite" title={editor.autosave.error?.message}>
						{#if s === 'saving'}
							<span aria-hidden="true">·</span> <span>{ru.save.saving}</span>
						{:else if s === 'saved'}
							<span aria-hidden="true">·</span> <span class="saved">✓ {ru.save.saved}</span>
						{:else if s === 'error' || s === 'conflict'}
							<span aria-hidden="true">·</span> <span class="failed">{ru.save.failed}</span>
							{#if s === 'error'}
								<span aria-hidden="true">·</span>
								<button type="button" class="retry" onclick={() => editor?.autosave.retry()}
									>{ru.common.retry}</button
								>
							{/if}
						{/if}
					</span>
				{/if}
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
					onclick={() => openIssue()}>{t.issue}</Button
				>
			{/if}
		{/if}
	{/snippet}
</SlideOver>

<IssueDialog
	bind:open={issueOpen}
	order={editor?.current ?? null}
	{dictionaries}
	defaultWarranty={app.settings.default_warranty}
	onissue={issue}
/>
<ReturnToWorkDialog bind:open={returnOpen} {order} statuses={backToWork} onreturn={returnToWork} />

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
	.save {
		display: contents;
	}
	.saved {
		color: var(--success-text);
	}
	.failed {
		color: var(--danger-text);
	}
	.retry {
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--accent);
		font: inherit;
		cursor: pointer;
	}
	.retry:hover {
		text-decoration: underline;
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
	@media (pointer: coarse) {
		.retry {
			min-height: 2.75rem;
		}
	}
</style>
