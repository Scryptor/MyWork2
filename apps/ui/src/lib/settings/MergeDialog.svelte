<!-- Merging the picked values of a dictionary: which one stays (the most used by default), and whether the
     orders get it too. The rest are deleted. -->
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
		rows,
		countLabel,
		where,
		onconfirm
	}: {
		open: boolean;
		/** `count: null` — not known. */
		rows: { id: number; label: string; count: number | null }[];
		/** «в 3 заказах» / «у 2 клиентов». */
		countLabel: (n: number) => string;
		/** «в заказах» / «у клиентов»: in «Заменить и …». */
		where: string;
		/** The text of a mistake, or null when it went through (the dialog closes). */
		onconfirm: (targetId: number, inOrders: boolean) => Promise<string | null>;
	} = $props();

	const t = ru.dictionaries;
	const uid = $props.id();
	let keep = $state<number | null>(null);
	let everywhere = $state(true);
	let busy = $state(false);
	let error = $state<string | null>(null);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			// The most used value stays; the first one among equals.
			keep =
				rows.reduce<(typeof rows)[number] | null>(
					(best, r) => (best === null || (r.count ?? -1) > (best.count ?? -1) ? r : best),
					null
				)?.id ?? null;
			everywhere = true;
			error = null;
		});
	});

	async function confirm() {
		if (busy || keep === null) return;
		busy = true;
		error = null;
		try {
			error = await onconfirm(keep, everywhere);
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
	title={t.mergeTitle}
	description={t.mergeDescription}
	size="sm"
	dismissible={!busy}
>
	<fieldset class="choice" disabled={busy}>
		<legend>{t.mergeKeep}</legend>
		{#each rows as r (r.id)}
			<label class="option">
				<input type="radio" name="{uid}-keep" value={r.id} bind:group={keep} />
				<span class="name">{r.label}</span>
				{#if r.count !== null}<span class="count">{countLabel(r.count)}</span>{/if}
			</label>
		{/each}
	</fieldset>
	<Checkbox bind:checked={everywhere} label={t.mergeEverywhere(where)} disabled={busy} />
	{#if error}<Callout tone="danger">{error}</Callout>{/if}
	{#snippet footer()}
		<Button variant="secondary" disabled={busy} onclick={() => (open = false)}>
			{ru.common.cancel}
		</Button>
		<Button variant="primary" {busy} onclick={() => void confirm()}>{t.merge}</Button>
	{/snippet}
</Dialog>

<style>
	.choice {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0 0 var(--space-3);
		padding: 0;
		border: 0;
	}
	legend {
		margin-bottom: var(--space-1);
		padding: 0;
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--text-2);
	}
	.option {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--control-md);
		font-size: var(--text-md);
	}
	.option input {
		margin: 0;
		accent-color: var(--accent);
	}
	.name {
		flex: 1;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.count {
		font-size: var(--text-sm);
		color: var(--text-3);
		white-space: nowrap;
	}
</style>
