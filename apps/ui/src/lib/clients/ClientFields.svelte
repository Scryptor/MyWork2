<!-- The fields of a client (spec 6.6): text is saved a moment after the last key or on leaving the field,
     choices, switches and changes of the phone list at once. The phones are one list: the first number is
     the main one, each has a note. A conflict marks the disputed fields like in an order. -->
<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { tick, type Snippet } from 'svelte';
	import type { Client } from '$api/Client';
	import type { ClientPhone } from '$api/ClientPhone';
	import { formatPhone } from '#lib/format/phone.js';
	import { ru } from '#lib/i18n/ru.js';
	import type { Autosave } from '#lib/orders/autosave.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import PhoneInput from '#lib/ui/PhoneInput.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import type { ClientConflict, ClientField, ClientValues } from './editor.svelte.js';
	import { sameValue } from '#lib/orders/merge.js';
	import { asPrimary, typedNumber } from './phones.js';

	let {
		client,
		autosave,
		sources,
		readonly = false,
		errors = {},
		conflicts = new Map(),
		flashed = new Set(),
		onkeep,
		ontake,
		onflashend
	}: {
		client: Client;
		autosave: Autosave<ClientValues>;
		/** The dictionary «Откуда узнали». */
		sources: string[];
		/** No editing (no right to edit clients). */
		readonly?: boolean;
		/** What the server said is wrong with a field. */
		errors?: Partial<Record<ClientField, string>>;
		conflicts?: ReadonlyMap<ClientField, ClientConflict>;
		flashed?: ReadonlySet<ClientField>;
		onkeep?: (field: ClientField) => void;
		ontake?: (field: ClientField) => void;
		onflashend?: (field: ClientField) => void;
	} = $props();

	const t = ru.client;
	const o = ru.order;
	const LABELS: Record<ClientField, string> = {
		full_name: t.fullName,
		phones: t.phones,
		address: t.address,
		note: t.note,
		source: t.source,
		blacklisted: t.blacklisted
	};

	/** What the field shows: own unsaved value, or the client's. */
	const shown = <K extends ClientField>(f: K): ClientValues[K] => autosave.value(f, client[f]);
	const sourceItems = $derived(sources.map((value) => ({ value })));

	// The phones. A row added by «＋ номер» stays here until it gets a number: an empty row is not sent.
	const phones = $derived(shown('phones'));
	let draft = $state<ClientPhone | null>(null);
	const rows = $derived(draft ? [...phones, draft] : phones);
	/** Disputed phones are only decided by «Оставить моё» / «Взять новое»: typing would settle them. */
	const phonesLocked = $derived(readonly || conflicts.has('phones'));
	const numberId = (i: number) => `client-phone-${i}`;

	async function focus(id: string) {
		await tick();
		document.getElementById(id)?.focus();
	}

	/** The row typed in now and its number when typing began (null: it was the row of «＋ номер»). */
	let typing: { index: number; before: string | null } | null = null;

	/**
	 * Typing a number. A number goes from MIN_PHONE_DIGITS digits on; erasing it with Backspace sends no
	 * piece of it: below that the row keeps its number from before typing (shown again on leaving the
	 * field), and only «Убрать номер» takes it away.
	 */
	function setNumber(i: number, digits: string) {
		if (typing?.index !== i) typing = { index: i, before: phones[i]?.phone ?? null };
		const next = typedNumber(phones, draft, i, digits, typing.before);
		draft = next.draft;
		if (!sameValue(next.phones, phones)) autosave.edit('phones', next.phones);
	}

	function setNote(i: number, note: string) {
		if (i < phones.length) autosave.edit('phones', phones.with(i, { ...phones[i]!, note }));
		else if (draft) draft = { ...draft, note };
	}

	function addPhone() {
		if (!draft) draft = { phone: '', note: '' };
		void focus(numberId(phones.length));
	}

	function removePhone(i: number) {
		if (i < phones.length) autosave.commit('phones', phones.toSpliced(i, 1));
		else draft = null;
		void focus(i > 0 ? numberId(i - 1) : 'client-add-phone');
	}

	function makePrimary(i: number) {
		autosave.commit('phones', asPrimary(phones, i));
		void focus(numberId(0));
	}

	/**
	 * Leaving a field saves what was typed in it; then it shows the value as the server keeps it. Another
	 * window taking focus (Alt-Tab) is not leaving.
	 */
	function leave(e: FocusEvent) {
		typing = null;
		void autosave.flush().catch(() => {});
		if (e.relatedTarget === null || !document.hasFocus()) return;
		autosave.release();
	}

	const flash = (f: ClientField) => flashed.has(f);

	function asText(f: ClientField, v: unknown): string {
		if (typeof v === 'boolean') return v ? o.yes : o.no;
		if (Array.isArray(v)) {
			const list = (v as ClientPhone[])
				.map((p) => [formatPhone(p.phone), p.note].filter(Boolean).join(' '))
				.join(', ');
			return list || o.empty;
		}
		if (v === null || v === '') return o.empty;
		return String(v);
	}
</script>

{#snippet dispute(f: ClientField)}
	{@const c = conflicts.get(f)}
	{#if c}
		<div class="dispute" role="group" aria-label={o.conflictField(LABELS[f])}>
			<p class="theirs"><span class="muted">{o.theirs}:</span> {asText(f, c.theirs)}</p>
			<div class="choices">
				<Button size="sm" onclick={() => onkeep?.(f)}>{o.keepMine}</Button>
				<Button size="sm" onclick={() => ontake?.(f)}>{o.takeNew}</Button>
			</div>
		</div>
	{/if}
{/snippet}

{#snippet flashable(f: ClientField, body: Snippet)}
	<div class="flashable" class:flash={flash(f)} onanimationend={() => onflashend?.(f)}>
		{@render body()}
		{@render dispute(f)}
	</div>
{/snippet}

<div class="fields" onfocusout={leave}>
	{#if conflicts.size > 0}
		<Callout tone="warning" title={t.conflictTitle}>{o.conflictText}</Callout>
	{/if}

	{#snippet name()}
		<TextInput
			id="client-full_name"
			label={t.fullName}
			autocomplete="off"
			disabled={readonly}
			error={errors.full_name}
			bind:value={() => shown('full_name'), (v) => autosave.edit('full_name', v)}
		/>
	{/snippet}
	{@render flashable('full_name', name)}

	{#snippet list()}
		{#each rows as p, i (i)}
			{@const real = i < phones.length}
			<div class="phone" role="group" aria-label={t.phoneGroup(i + 1)}>
				<PhoneInput
					id={numberId(i)}
					label={i === 0 ? t.phone : t.phoneN(i + 1)}
					hint={i === 0 && rows.length > 1 ? t.primary : undefined}
					disabled={phonesLocked}
					bind:value={() => p.phone, (v) => setNumber(i, v)}
				/>
				<div class="note">
					<TextInput
						id="client-phone-note-{i}"
						label={i === 0 ? t.phoneNote : t.phoneNoteN(i + 1)}
						autocomplete="off"
						disabled={phonesLocked}
						bind:value={() => p.note, (v) => setNote(i, v)}
					/>
					{#if !phonesLocked}
						<IconButton label={t.removePhone(i + 1)} icon={X} onclick={() => removePhone(i)} />
					{/if}
				</div>
				{#if i > 0 && real && !phonesLocked}
					<div class="primary">
						<Button size="sm" variant="ghost" onclick={() => makePrimary(i)}>{t.makePrimary}</Button
						>
					</div>
				{/if}
			</div>
		{/each}
		{#if errors.phones}<p class="error" role="alert">{errors.phones}</p>{/if}
		{#if !phonesLocked}
			<div>
				<Button id="client-add-phone" size="sm" variant="ghost" onclick={addPhone}
					>{t.addPhone}</Button
				>
			</div>
		{/if}
	{/snippet}
	<Section title={t.phones}>
		{@render flashable('phones', list)}
	</Section>

	{#snippet address()}
		<TextInput
			id="client-address"
			label={t.address}
			autocomplete="off"
			disabled={readonly}
			bind:value={() => shown('address'), (v) => autosave.edit('address', v)}
		/>
	{/snippet}
	{@render flashable('address', address)}

	{#snippet source()}
		<Combobox
			id="client-source"
			label={t.source}
			items={sourceItems}
			disabled={readonly}
			bind:value={() => shown('source'), (v) => autosave.edit('source', v)}
			onselect={(v) => autosave.commit('source', v)}
		/>
	{/snippet}
	{@render flashable('source', source)}

	{#snippet note()}
		<Textarea
			id="client-note"
			label={t.note}
			minRows={2}
			disabled={readonly}
			bind:value={() => shown('note'), (v) => autosave.edit('note', v)}
		/>
	{/snippet}
	{@render flashable('note', note)}

	{#snippet blacklisted()}
		<Switch
			label={t.blacklisted}
			disabled={readonly}
			bind:checked={() => shown('blacklisted'), (v) => autosave.commit('blacklisted', v)}
		/>
	{/snippet}
	{@render flashable('blacklisted', blacklisted)}
</div>

<style>
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.phone {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		align-items: start;
		gap: var(--space-1) var(--space-3);
	}
	.note {
		display: flex;
		align-items: flex-end;
		gap: var(--space-1);
	}
	.note > :global(:first-child) {
		flex: 1 1 auto;
		min-width: 0;
	}
	.primary {
		grid-column: 1 / -1;
		margin-top: calc(-1 * var(--space-1));
	}
	.muted {
		color: var(--text-3);
	}
	.error {
		margin: 0;
		color: var(--danger-text);
		font-size: var(--text-sm);
	}
	.flashable {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
		border-radius: var(--radius-md);
	}
	/* Taken from another computer: a short highlight (spec 6.2). */
	.flash {
		animation: flash var(--duration-flash) var(--ease-out);
	}
	@keyframes flash {
		from {
			background: var(--warning-soft);
			box-shadow: 0 0 0 var(--space-1) var(--warning-soft);
		}
	}
	.dispute {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--warning-soft);
		color: var(--warning-text);
		font-size: var(--text-sm);
	}
	.theirs {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.choices {
		display: flex;
		gap: var(--space-2);
	}
	:global([data-width='xs']) .phone {
		grid-template-columns: minmax(0, 1fr);
	}
</style>
