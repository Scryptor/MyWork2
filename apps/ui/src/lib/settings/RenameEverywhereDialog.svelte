<!-- A renamed value that stands in orders: «Переименовать „старое“ в „новое“» with the choice to change the
     orders too. The request can take a moment (hundreds of orders): the button is busy, the window stays. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Checkbox from '#lib/ui/Checkbox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let {
		open = $bindable(false),
		from,
		to,
		/** «в 3 заказах» / «у 2 клиентов»: how many records have the value. */
		where,
		/** A warranty: the orders keep their term, so the change in them is off by default. */
		warranty = false,
		onconfirm
	}: {
		open: boolean;
		from: string;
		to: string;
		where: string;
		warranty?: boolean;
		/** The text of a mistake, or null when it went through (the dialog closes). */
		onconfirm: (inOrders: boolean) => Promise<string | null>;
	} = $props();

	const t = ru.dictionaries;
	let everywhere = $state(true);
	let busy = $state(false);
	let error = $state<string | null>(null);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			everywhere = !warranty;
			error = null;
		});
	});

	async function confirm() {
		if (busy) return;
		busy = true;
		error = null;
		try {
			error = await onconfirm(everywhere);
			if (error === null) open = false;
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		if (open) return submitShortcut(() => void confirm());
	});
</script>

<Dialog
	bind:open
	title={t.renameTitle(from, to)}
	description={t.renameDescription}
	size="sm"
	dismissible={!busy}
>
	<Checkbox bind:checked={everywhere} label={t.renameEverywhere(where)} disabled={busy} />
	{#if warranty}<p class="note">{t.warrantyNote}</p>{/if}
	{#if error}<Callout tone="danger">{error}</Callout>{/if}
	{#snippet footer()}
		<Button variant="secondary" disabled={busy} onclick={() => (open = false)}>
			{ru.common.cancel}
		</Button>
		<Button variant="primary" {busy} onclick={() => void confirm()}>{t.rename}</Button>
	{/snippet}
</Dialog>

<style>
	.note {
		margin: var(--space-2) 0 0;
		font-size: var(--text-sm);
		color: var(--text-3);
	}
</style>
