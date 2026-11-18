<!-- The client panel (spec 6.6): the client's fields saved by themselves, «Похожие клиенты», the client's
     orders (each opens the order panel over this one, with «← к клиенту») and «Новый заказ». On a phone it
     fills the screen. Which client is open lives in the address of «Клиенты». -->
<script lang="ts">
	import Phone from '@lucide/svelte/icons/phone';
	import Plus from '@lucide/svelte/icons/plus';
	import { untrack } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { beforeNavigate } from '$app/navigation';
	import type { Client } from '$api/Client';
	import type { ClientListItem } from '$api/ClientListItem';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { errorMessage } from '#lib/api/client.js';
	import { getClient, similarClients } from '#lib/api/clients.js';
	import { app } from '#lib/app/context.svelte.js';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { countLabel, dialNumber, displayName } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import { intake } from '#lib/intake/intake.svelte.js';
	import { orderPanel, type PanelOptions } from '#lib/orders/panel.svelte.js';
	import SaveIndicator from '#lib/orders/SaveIndicator.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import SlideOver from '#lib/ui/SlideOver.svelte';
	import ClientFields from './ClientFields.svelte';
	import ClientOrders from './ClientOrders.svelte';
	import { phonesText } from './columns.js';
	import {
		clientFieldErrors,
		clientTitle,
		closeClientEditor,
		createClientEditor,
		type ClientEditor
	} from './editor.svelte.js';

	let {
		clientId,
		orderBack,
		onclose,
		onopenclient,
		onchanged
	}: {
		/** The client open (from the address); null: the panel is closed. */
		clientId: number | null;
		/** «← к клиенту» of an order opened from here: back to this client as the list shows it now. */
		orderBack: (id: number) => NonNullable<PanelOptions['back']>;
		/** The panel asks to be closed; its edits are saved by then. */
		onclose: () => void;
		/** A similar client clicked: their panel. */
		onopenclient: (id: number) => void;
		/** Every answer of the server about the client: the list catches up. */
		onchanged?: (c: Client) => void;
	} = $props();

	const t = ru.client;
	const phone = new MediaQuery(PHONE_QUERY);

	/** The client shown, one editor per client; another client gets a new one. */
	let editor = $state.raw<ClientEditor | null>(null);
	let loadError = $state<string | null>(null);
	let sources = $state.raw<string[]>([]);
	let similar = $state.raw<ClientListItem[]>([]);

	const client = $derived(editor?.client ?? null);
	const canEdit = $derived(app.can('clients_edit') || app.can('orders_edit'));
	const canIntake = $derived(app.can('orders_create'));
	const errors = $derived(clientFieldErrors(editor?.autosave.error ?? null));
	const mainPhone = $derived(client?.phones[0]?.phone ?? '');
	const title = $derived(client ? clientTitle(client) : t.number(clientId ?? 0));

	// «Откуда узнали», asked again at every opening: settings may have changed.
	const isOpen = $derived(clientId !== null);
	$effect(() => {
		if (!isOpen) return;
		let current = true;
		listDictionary('client_source').then(
			(items) => {
				if (current) sources = items.map((d) => d.value);
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
		const id = clientId;
		untrack(() => void load(id));
	});

	async function load(id: number | null) {
		const mine = ++token;
		if (id === null) {
			swap(null);
			loadError = null;
			return;
		}
		if (editor?.client.id === id) return;
		loadError = null;
		similar = [];
		try {
			const c = await getClient(id);
			if (mine !== token) return;
			swap(createClientEditor(c, { emit: (saved) => onchanged?.(saved) }));
		} catch (e) {
			if (mine !== token) return;
			swap(null);
			loadError = errorMessage(e);
		}
	}

	function swap(next: ClientEditor | null) {
		const old = editor;
		editor = next;
		// What was typed is still saved; a failure is told.
		if (old) closeClientEditor(old);
	}

	// «Похожие клиенты»: when the client opens and after the name or a phone is saved.
	const similarKey = $derived(
		client ? JSON.stringify([client.id, client.full_name, client.phones]) : null
	);
	$effect(() => {
		const key = similarKey;
		const id = client?.id;
		if (key === null || id === undefined) return;
		let current = true;
		similarClients(id).then(
			(list) => {
				if (current) similar = list.filter((s) => s.id !== id);
			},
			() => {
				// No hint without an answer.
			}
		);
		return () => {
			current = false;
		};
	});

	/** Before another client is shown or the panel closes: own edits are saved; false keeps the panel. */
	export function leave(): Promise<boolean> {
		return editor ? editor.flush() : Promise.resolve(true);
	}

	async function close() {
		if (await leave()) onclose();
	}

	// Closing or reloading the tab while edits wait or failed gets the browser's own question.
	beforeNavigate((nav) => {
		if (nav.willUnload && editor?.unsaved) nav.cancel();
	});
	// The screen goes: what is typed is still sent, a failure is told.
	$effect(
		() => () =>
			untrack(() => {
				if (editor) closeClientEditor(editor);
			})
	);

	function openOrder(id: number) {
		const c = client;
		if (!c) return;
		void orderPanel.open(id, { back: orderBack(c.id) });
	}
</script>

<SlideOver bind:open={() => clientId !== null, (v) => !v && void close()} {title}>
	{#snippet header()}
		{#if client && editor}
			<p class="meta num">
				<span>{t.number(client.id)}</span>
				<SaveIndicator {editor} />
			</p>
		{/if}
	{/snippet}

	<div class="content">
		{#if editor && client}
			{@const ed = editor}
			{#key ed}
				{#if similar.length > 0}
					<Callout tone="info" title={t.similar(similar.length)}>
						<p class="hint">{t.similarHint}</p>
						<ul class="similar">
							{#each similar as s (s.id)}
								<li>
									<button type="button" class="link" onclick={() => onopenclient(s.id)}>
										<span>{displayName(s.full_name) || t.noName}</span>
										<span class="muted num"
											>{[phonesText(s.phones), countLabel(s.orders_count, ru.orders.forms)]
												.filter(Boolean)
												.join(' · ')}</span
										>
									</button>
								</li>
							{/each}
						</ul>
					</Callout>
				{/if}
				<ClientFields
					client={ed.client}
					autosave={ed.autosave}
					{sources}
					{errors}
					readonly={!canEdit}
					conflicts={ed.conflicts}
					flashed={ed.flashed}
					onkeep={(f) => ed.keep(f)}
					ontake={(f) => ed.take(f)}
					onflashend={(f) => ed.endFlash(f)}
				/>
				<ClientOrders clientId={client.id} onopen={openOrder} />
			{/key}
		{:else if loadError}
			<Callout tone="danger" title={t.loadFailed}>
				{loadError}
				{#snippet actions()}
					<Button size="sm" onclick={() => void load(clientId)}>{ru.common.retry}</Button>
				{/snippet}
			</Callout>
		{:else}
			<div class="loading" aria-busy="true">
				<Skeleton height="2.5rem" />
				<Skeleton height="4rem" />
				<Skeleton height="2.5rem" width="80%" />
			</div>
		{/if}
	</div>

	{#snippet footer()}
		{#if phone.current && mainPhone}
			<Button href="tel:{dialNumber(mainPhone)}" icon={Phone}>{ru.order.call}</Button>
		{/if}
		<span class="grow"></span>
		{#if client && canIntake}
			<Button variant="primary" icon={Plus} onclick={() => intake.open({ clientId: client.id })}
				>{t.newOrder}</Button
			>
		{/if}
	{/snippet}
</SlideOver>

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
	.hint {
		margin: 0 0 var(--space-1);
		color: var(--text-2);
		font-size: var(--text-sm);
	}
	.similar {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.link {
		display: inline-flex;
		flex-wrap: wrap;
		column-gap: var(--space-2);
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--accent);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.link:hover {
		text-decoration: underline;
	}
	.muted {
		color: var(--text-3);
	}
	.grow {
		flex: 1;
	}
	@media (pointer: coarse) {
		.link {
			min-height: 2.75rem;
		}
	}
</style>
