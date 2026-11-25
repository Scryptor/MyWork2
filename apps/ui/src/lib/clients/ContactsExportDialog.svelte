<!-- «Контакты для телефона»: the format and which clients of the list (all, or those added since a day).
     The day of the last export is kept on this computer and offered next time; a refusal of the server
     (no phones, too many rows) stands under the form in the server's words. -->
<script lang="ts">
	import type { ClientExportQuery } from '$api/ClientExportQuery';
	import type { ContactsFormat } from '$api/ContactsFormat';
	import { errorReason } from '#lib/api/client.js';
	import { downloadContacts } from '#lib/api/exports.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { isoDay } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import DateInput from '#lib/ui/DateInput.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';

	let { open = $bindable(false), query }: { open: boolean; query: ClientExportQuery } = $props();

	const t = ru.exports.contacts;
	/** The day of the last successful export (the enterprise's day by the server's clock), kept in the browser of this computer. */
	const STORAGE_KEY = 'mw.contactsExportedOn';

	function lastExportDay(): string | null {
		try {
			const day = localStorage.getItem(STORAGE_KEY);
			return day && /^\d{4}-\d{2}-\d{2}$/.test(day) ? day : null;
		} catch {
			return null;
		}
	}

	function rememberToday() {
		try {
			localStorage.setItem(STORAGE_KEY, isoDay(serverClock.now()));
		} catch {
			// No storage (a private window): the next time simply starts from «Все из списка».
		}
	}

	let format = $state<ContactsFormat>('csv');
	let scope = $state<'all' | 'since'>('all');
	let since = $state<string | null>(null);
	let sinceInvalid = $state(false);
	let error = $state<string | null>(null);
	let busy = $state(false);

	// Every opening starts afresh, with the day of the last export if there is one.
	$effect(() => {
		if (!open) return;
		const day = lastExportDay();
		scope = day ? 'since' : 'all';
		since = day;
		sinceInvalid = false;
		error = null;
		busy = false;
	});

	const ready = $derived(!busy && (scope === 'all' || (since !== null && !sinceInvalid)));

	async function submit() {
		if (!ready) return;
		busy = true;
		error = null;
		try {
			await downloadContacts(query, format, scope === 'since' ? since : null);
			rememberToday();
			open = false;
		} catch (e) {
			error = errorReason(e);
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});
</script>

<Dialog bind:open title={t.title} description={t.description} dismissible={!busy}>
	<div class="form">
		<SegmentedControl
			bind:value={format}
			label={t.format}
			options={[
				{ value: 'csv', label: t.csv },
				{ value: 'vcard', label: t.vcard }
			]}
		/>
		<SegmentedControl
			bind:value={scope}
			label={t.clients}
			options={[
				{ value: 'all', label: t.all },
				{ value: 'since', label: t.since }
			]}
		/>
		{#if scope === 'since'}
			<DateInput bind:value={since} bind:invalid={sinceInvalid} label={t.sinceDate} />
		{/if}
		{#if error}
			<Callout tone="danger">{error}</Callout>
		{/if}
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)} disabled={busy}>{ru.common.cancel}</Button>
		<Button variant="primary" {busy} disabled={!ready} onclick={() => void submit()}
			>{t.submit}</Button
		>
	{/snippet}
</Dialog>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
</style>
