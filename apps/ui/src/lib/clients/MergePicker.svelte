<!-- «Объединить с…» of the client panel: finds the other client by name; «Далее» goes on to the merge. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { listClients } from '#lib/api/clients.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { displayName } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { phonesText } from './columns.js';

	/** The search asks the server once typing pauses for this long. */
	const SEARCH_MS = 200;

	let {
		open = $bindable(false),
		clientId,
		onpick
	}: {
		open: boolean;
		/** The client of the panel: not offered. */
		clientId: number;
		/** The other client, with the number of their orders the search has already shown. */
		onpick: (otherId: number, ordersCount: number) => void;
	} = $props();

	const t = ru.merge;
	let text = $state('');
	let found = $state.raw<{ value: string; hint?: string; id: number; count: number }[]>([]);
	/** Every label offered so far and its client: a label typed or picked is known even after a new search. */
	let known = $state.raw(new Map<string, { id: number; count: number }>());
	const picked = $derived(known.get(text) ?? null);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let token = 0;

	$effect(() => {
		if (open) {
			untrack(() => {
				text = '';
				found = [];
				known = new Map();
			});
		}
		return () => clearTimeout(timer);
	});

	function search(query: string) {
		clearTimeout(timer);
		const mine = ++token;
		if (query.trim() === '') {
			found = [];
			return;
		}
		timer = setTimeout(() => {
			listClients({ query, limit: 20 }).then(
				(page) => {
					if (mine !== token) return;
					found = page.items
						.filter((c) => c.id !== clientId)
						.map((c) => ({
							id: c.id,
							count: c.orders_count,
							value: `${displayName(c.full_name) || ru.client.noName} · ${ru.client.number(c.id)}`,
							hint: phonesText(c.phones) || undefined
						}));
					known = new Map([
						...known,
						...found.map((f) => [f.value, { id: f.id, count: f.count }] as const)
					]);
				},
				() => {
					// No suggestions without an answer.
				}
			);
		}, SEARCH_MS);
	}

	$effect(() => {
		const v = text;
		untrack(() => search(v));
	});

	function next() {
		if (picked === null) return;
		const { id, count } = picked;
		open = false;
		onpick(id, count);
	}
	$effect(() => {
		if (open && picked !== null) return submitShortcut(next);
	});
</script>

<Dialog bind:open title={t.pickerTitle} size="sm">
	<Combobox
		bind:value={text}
		items={found}
		filter={false}
		label={t.pickerClient}
		placeholder={t.pickerPlaceholder}
	/>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" disabled={picked === null} onclick={next}>{t.next}</Button>
	{/snippet}
</Dialog>
