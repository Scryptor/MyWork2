<!-- «Клиент» of the intake (spec 6.5): the phone first; the clients with that number as cards to choose
     from, or the fields of a new client. -->
<script lang="ts" module>
	/** The first field of the intake. */
	export const PHONE_ID = 'intake-phone';
</script>

<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { tick, untrack } from 'svelte';
	import type { Client } from '$api/Client';
	import type { ClientPhone } from '$api/ClientPhone';
	import { errorMessage } from '#lib/api/client.js';
	import { clientsByPhone, getClient } from '#lib/api/clients.js';
	import { listOrders } from '#lib/api/orders.js';
	import { countLabel, displayName, formatPhone } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import Avatar from '#lib/ui/Avatar.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import PhoneInput from '#lib/ui/PhoneInput.svelte';
	import Section from '#lib/ui/Section.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { phoneQuery, type IntakeDraft } from './draft.js';

	let {
		draft = $bindable(),
		sources,
		errors,
		onchosen
	}: {
		draft: IntakeDraft;
		/** «Откуда узнали» of the dictionary. */
		sources: string[];
		errors: { contact?: string; phones?: string; client?: string };
		/** A client was chosen (their number): the form goes on to the device. */
		onchosen?: (clientId: number) => void;
	} = $props();

	const NAME_ID = 'intake-name';
	/** Pause after the last digit before the lookup. */
	const LOOKUP_MS = 300;
	const t = ru.intake;

	interface Found {
		client: Client;
		/** The number of their orders; null until counted. */
		orders: number | null;
	}
	let found = $state.raw<Found[]>([]);
	let chosen = $state.raw<Client | null>(null);
	let chosenError = $state<string | null>(null);
	let addressOpen = $state(untrack(() => draft.address !== ''));

	// The clients with this phone, a moment after the last digit; a newer number aborts the older lookup.
	// The cards of the old number go at once: they are not about the number typed. While a client is
	// chosen nothing is asked, and «Другой клиент» shows the cards again at once.
	const query = $derived(phoneQuery(draft));
	const choosing = $derived(draft.clientId === null);
	let foundFor: string | null = null;
	$effect(() => {
		const q = query;
		if (q !== foundFor) {
			foundFor = q;
			found = [];
		}
		if (q === null || !choosing) return;
		const controller = new AbortController();
		const timer = setTimeout(() => void lookUp(q, controller.signal), LOOKUP_MS);
		return () => {
			clearTimeout(timer);
			controller.abort();
		};
	});

	async function lookUp(phone: string, signal: AbortSignal) {
		let clients: Client[];
		try {
			clients = await clientsByPhone(phone, signal);
		} catch {
			// No connection: the lookup is a help, the form works without it (and shows no cards of an
			// older number). Aborted: a newer lookup is on its way.
			if (!signal.aborted) found = [];
			return;
		}
		if (signal.aborted) return;
		found = clients.map((client) => ({ client, orders: null }));
		const counts = await Promise.all(
			clients.map((c) =>
				listOrders({ client_id: c.id, limit: 1 }, signal).then(
					(p) => p.total,
					() => null
				)
			)
		);
		if (!signal.aborted)
			found = clients.map((client, i) => ({ client, orders: counts[i] ?? null }));
	}

	// The chosen client as the server has them (also for a draft or a prefill that has only the number).
	$effect(() => {
		const id = draft.clientId;
		if (id === null) {
			chosen = null;
			chosenError = null;
			return;
		}
		if (untrack(() => chosen?.id) === id) return;
		let current = true;
		chosenError = null;
		getClient(id).then(
			(c) => {
				if (current) chosen = c;
			},
			(e: unknown) => {
				if (current) chosenError = t.clientFailed(errorMessage(e));
			}
		);
		return () => {
			current = false;
		};
	});

	const showFound = $derived(draft.clientId === null && found.length > 0 && !draft.newClient);
	const newForm = $derived(draft.clientId === null && !showFound);

	function choose(c: Client) {
		chosen = c;
		chosenError = null;
		draft.clientId = c.id;
		draft.newClient = false;
		onchosen?.(c.id);
	}

	/** «+7 921 123-45-67 · жена · основной». */
	function phoneLine(p: ClientPhone, main: boolean): string {
		return [formatPhone(p.phone), p.note, main ? t.mainPhone : ''].filter(Boolean).join(' · ');
	}

	async function focus(id: string) {
		await tick();
		document.getElementById(id)?.focus();
	}

	function other() {
		draft.clientId = null;
		void focus(PHONE_ID);
	}

	function newClient() {
		draft.newClient = true;
		void focus(NAME_ID);
	}

	function addPhone() {
		draft.phones.push({ phone: '', note: '' });
		void focus(`intake-phone-${draft.phones.length - 1}`);
	}

	function removePhone(i: number) {
		draft.phones.splice(i, 1);
		void focus(PHONE_ID);
	}
</script>

{#snippet card(c: Client, orders: number | null)}
	<div class="head">
		<Avatar name={c.full_name || formatPhone(c.phones[0]?.phone ?? '')} />
		<strong>{displayName(c.full_name) || t.noName}</strong>
	</div>
	{#if c.blacklisted}<Callout tone="danger">{t.blacklisted}</Callout>{/if}
	<div class="phones">
		{#each c.phones as p, i (i)}
			<span class="num">{phoneLine(p, i === 0 && c.phones.length > 1)}</span>
		{/each}
	</div>
	{#if orders !== null || c.source}
		<p class="muted">
			{[
				orders === null ? '' : countLabel(orders, ru.orders.forms),
				c.source ? t.sourceLine(c.source) : ''
			]
				.filter(Boolean)
				.join(' · ')}
		</p>
	{/if}
	{#if c.note}<p class="note">{c.note}</p>{/if}
{/snippet}

<Section title={t.client}>
	{#if draft.clientId !== null}
		<section class="client" aria-label={t.chosen}>
			{#if chosen}
				{@render card(chosen, null)}
			{:else if chosenError}
				<p class="error">{chosenError}</p>
			{:else}
				<p class="muted">{ru.common.loading}</p>
			{/if}
			{#if errors.client}<p class="error" role="alert">{errors.client}</p>{/if}
			<div class="actions">
				<Button size="sm" variant="ghost" onclick={other}>{t.otherClient}</Button>
			</div>
		</section>
	{:else}
		<div class="grid two">
			<PhoneInput
				id={PHONE_ID}
				label={t.phone}
				bind:value={draft.phones[0]!.phone}
				error={errors.contact ?? errors.phones}
			/>
			{#if newForm}
				<TextInput label={t.phoneNote} bind:value={draft.phones[0]!.note} />
			{/if}
		</div>

		{#if showFound}
			<ul class="found" aria-label={t.found}>
				{#each found as f (f.client.id)}
					<li class="client">
						{@render card(f.client, f.orders)}
						<div class="actions">
							<Button size="sm" variant="primary" onclick={() => choose(f.client)}
								>{t.itsThem}</Button
							>
						</div>
					</li>
				{/each}
			</ul>
			<div>
				<Button size="sm" onclick={newClient}>{t.newClient}</Button>
			</div>
		{:else}
			{#if found.length > 0}
				<div>
					<Button size="sm" variant="ghost" onclick={() => (draft.newClient = false)}
						>{t.showFound(countLabel(found.length, t.clientForms))}</Button
					>
				</div>
			{/if}
			{#each draft.phones as p, i (i)}
				{#if i > 0}
					<div class="grid two">
						<PhoneInput id="intake-phone-{i}" label={t.phoneN(i + 1)} bind:value={p.phone} />
						<div class="note-row">
							<TextInput label={t.phoneNoteN(i + 1)} bind:value={p.note} />
							<IconButton label={t.removePhone(i + 1)} icon={X} onclick={() => removePhone(i)} />
						</div>
					</div>
				{/if}
			{/each}
			<div>
				<Button size="sm" variant="ghost" onclick={addPhone}>{t.addPhone}</Button>
			</div>
			<div class="grid two">
				<TextInput
					id={NAME_ID}
					label={t.fullName}
					bind:value={draft.fullName}
					error={errors.contact}
				/>
				<Combobox
					label={t.source}
					items={sources.map((value) => ({ value }))}
					bind:value={draft.source}
				/>
			</div>
			{#if addressOpen}
				<TextInput label={t.address} bind:value={draft.address} />
			{:else}
				<div>
					<Button
						size="sm"
						variant="ghost"
						aria-expanded="false"
						onclick={() => (addressOpen = true)}>{t.more}</Button
					>
				</div>
			{/if}
		{/if}
	{/if}
</Section>

<style>
	.grid {
		display: grid;
		gap: var(--space-3);
	}
	.grid.two {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}
	.note-row {
		display: flex;
		align-items: flex-end;
		gap: var(--space-2);
	}
	.note-row > :global(:first-child) {
		flex: 1 1 auto;
	}
	.found {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.client {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}
	.client p {
		margin: 0;
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.phones {
		display: flex;
		flex-direction: column;
	}
	.muted {
		color: var(--text-3);
	}
	.note {
		white-space: pre-line;
	}
	.error {
		color: var(--danger-text);
	}
	.actions {
		display: flex;
		gap: var(--space-2);
	}
	@media (max-width: 599px) {
		.grid.two {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
