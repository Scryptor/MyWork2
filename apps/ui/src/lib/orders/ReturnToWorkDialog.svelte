<!-- «Вернуть в работу» of an issued order: a status that is not issued, by default the one before the issue. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { Order } from '$api/Order';
	import type { Status } from '$api/Status';
	import { errorMessage } from '#lib/api/client.js';
	import { orderHistory } from '#lib/api/orders.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Select from '#lib/ui/Select.svelte';
	import { statusBeforeIssue } from './history.js';

	/** The status before the issue is looked for among this many newest entries. */
	const HISTORY_LIMIT = 50;

	let {
		open = $bindable(false),
		order,
		statuses,
		onreturn
	}: {
		open: boolean;
		order: Order | null;
		/** Statuses to choose from: not issued, not archived, in their order. */
		statuses: Status[];
		/** Sets the status; throws the server's error, which the dialog shows. */
		onreturn: (statusId: number) => Promise<void>;
	} = $props();

	const t = ru.order;
	const items = $derived(statuses.map((s) => ({ value: s.id, label: s.name })));
	let chosen = $state<number | null>(null);
	let touched = false;
	let failure = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		if (!open) return;
		// Every opening starts over; a newer version of the order while open changes nothing here.
		const o = untrack(() => {
			failure = null;
			busy = false;
			touched = false;
			chosen = statuses[0]?.id ?? null;
			return order;
		});
		if (!o) return;
		let current = true;
		orderHistory(o.id, { limit: HISTORY_LIMIT }).then(
			(page) => {
				const before = statusBeforeIssue(
					page.items,
					o.status_id,
					statuses.map((s) => s.id)
				);
				if (current && !touched && before !== null) chosen = before;
			},
			() => {
				// Without the history the first status stays chosen.
			}
		);
		return () => {
			current = false;
		};
	});

	async function submit() {
		if (busy || chosen === null) return;
		busy = true;
		failure = null;
		try {
			await onreturn(chosen);
		} catch (e) {
			failure = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});
</script>

<Dialog bind:open title={t.returnTitle} size="sm">
	<div class="body">
		{#if failure}<Callout tone="danger">{failure}</Callout>{/if}
		<Select
			label={ru.status.label}
			hint={t.returnHint}
			{items}
			bind:value={
				() => chosen,
				(v) => {
					chosen = v;
					touched = true;
				}
			}
		/>
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button
			variant="primary"
			shortcut="Mod+Enter"
			{busy}
			disabled={chosen === null}
			onclick={() => void submit()}>{t.returnSubmit}</Button
		>
	{/snippet}
</Dialog>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
</style>
