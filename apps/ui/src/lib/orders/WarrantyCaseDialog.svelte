<!-- «Принять по гарантии» of an issued order (spec 6.4): the complaint and the day the client came back,
     today by default. Ctrl/⌘+Enter accepts. The server's errors show at their fields; one about the
     status (no status «Принят по гарантии», the order is not issued) shows above the form. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { NewWarrantyCase } from '$api/NewWarrantyCase';
	import type { Order } from '$api/Order';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { isoDay, startOfDay } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import DateInput from '#lib/ui/DateInput.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';

	let {
		open = $bindable(false),
		order,
		onaccept
	}: {
		open: boolean;
		order: Order | null;
		/** Accepts the case; throws the server's error, which the dialog shows. */
		onaccept: (c: NewWarrantyCase) => Promise<void>;
	} = $props();

	const t = ru.order;
	const IDS = { complaint: 'warranty-complaint', at: 'warranty-date' } as const;
	type Key = keyof typeof IDS;

	const today = () => isoDay(serverClock.now());

	let complaint = $state('');
	let date = $state<string | null>(null);
	let max = $state('');
	let dateInvalid = $state(false);
	/** «Принять» was pressed: an empty complaint shows as an error from now on. */
	let tried = $state(false);
	let serverErrors = $state<Partial<Record<Key, string>>>({});
	let failure = $state<string | null>(null);
	let busy = $state(false);

	// Every opening starts empty, with today.
	$effect.pre(() => {
		if (!open) return;
		untrack(() => {
			complaint = '';
			max = today();
			date = max;
			dateInvalid = false;
			tried = false;
			serverErrors = {};
			failure = null;
			busy = false;
		});
	});

	const missing = $derived({ complaint: complaint.trim() === '', at: date === null });
	const errors = $derived<Partial<Record<Key, string>>>({
		complaint: tried && missing.complaint ? t.complaintRequired : serverErrors.complaint,
		at: tried && missing.at && !dateInvalid ? ru.fields.dateError : serverErrors.at
	});

	function drop(k: Key) {
		if (serverErrors[k]) serverErrors = { ...serverErrors, [k]: undefined };
	}

	async function accept() {
		if (busy || !order) return;
		tried = true;
		const first = missing.complaint ? 'complaint' : missing.at || dateInvalid ? 'at' : null;
		if (first) {
			document.getElementById(IDS[first])?.focus();
			return;
		}
		busy = true;
		serverErrors = {};
		failure = null;
		const c: NewWarrantyCase = { complaint: complaint.trim() };
		// Today is «now» on the server; another day is that day.
		if (date !== null && date !== today()) c.at = startOfDay(date);
		try {
			await onaccept(c);
		} catch (e) {
			const fields = e instanceof ApiError && e.code === 'validation' ? (e.body?.fields ?? []) : [];
			const shown: Partial<Record<Key, string>> = {};
			const above: string[] = [];
			for (const f of fields) {
				if (f.field === 'complaint' || f.field === 'at') shown[f.field] ??= f.message;
				else above.push(f.message);
			}
			serverErrors = shown;
			const known = Object.keys(shown) as Key[];
			if (above.length > 0) failure = above.join('. ');
			else if (known.length === 0) failure = errorMessage(e);
			if (known.length > 0) document.getElementById(IDS[known[0]!])?.focus();
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		if (open) return submitShortcut(() => void accept());
	});
</script>

<Dialog bind:open title={t.warrantyTitle(order?.id ?? 0)}>
	<div class="body">
		{#if failure}<Callout tone="danger">{failure}</Callout>{/if}
		<Textarea
			id={IDS.complaint}
			label={t.complaint}
			help={ru.help.order.complaint}
			required
			bind:value={
				() => complaint,
				(v) => {
					complaint = v;
					drop('complaint');
				}
			}
			error={errors.complaint}
		/>
		<DateInput
			id={IDS.at}
			label={t.warrantyDate}
			required
			{max}
			bind:value={
				() => date,
				(v) => {
					date = v;
					drop('at');
				}
			}
			bind:invalid={dateInvalid}
			error={errors.at}
		/>
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" shortcut="Mod+Enter" {busy} onclick={() => void accept()}
			>{t.warrantyAccept}</Button
		>
	{/snippet}
</Dialog>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
</style>
