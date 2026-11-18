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
		onpick: (otherId: number) => void;
	} = $props();

	const t = ru.merge;
	let text = $state('');
	let found = $state.raw<{ value: string; hint?: string; id: number }[]>([]);
	let picked = $state<number | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let token = 0;

	$effect(() => {
		if (open) {
			untrack(() => {
				text = '';
				found = [];
				picked = null;
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
							value: `${displayName(c.full_name) || ru.client.noName} · ${ru.client.number(c.id)}`,
							hint: phonesText(c.phones) || undefined
						}));
				},
				() => {
					// No suggestions without an answer.
				}
			);
		}, SEARCH_MS);
	}

	function typed(v: string) {
		picked = found.find((f) => f.value === v)?.id ?? null;
		search(v);
	}
	$effect(() => {
		const v = text;
		untrack(() => typed(v));
	});

	function next() {
		if (picked === null) return;
		const id = picked;
		open = false;
		onpick(id);
	}
	$effect(() => {
		if (open && picked !== null) return submitShortcut(next);
	});
</script>

<Dialog bind:open title={t.pickerTitle} size="sm">
	<Combobox
		bind:value={text}
		items={found}
		label={t.pickerClient}
		placeholder={t.pickerPlaceholder}
	/>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" disabled={picked === null} onclick={next}>{t.next}</Button>
	{/snippet}
</Dialog>
