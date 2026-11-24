<!-- «История» of a position: the newest entries of its journal; the text of each is the server's summary.
     A new version of the position is a new entry: the list is asked again. -->
<script lang="ts">
	import type { AuditEntry } from '$api/AuditEntry';
	import { stockItemHistory } from '#lib/api/stock.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { historyItems } from '#lib/orders/history.js';
	import Section from '#lib/ui/Section.svelte';
	import Timeline from '#lib/ui/Timeline.svelte';

	const LIMIT = 10;

	let { itemId, version }: { itemId: number; version: number } = $props();

	const t = ru.stock;
	let entries = $state.raw<AuditEntry[] | null>(null);
	let failed = $state(false);

	$effect(() => {
		const id = itemId;
		void version;
		let current = true;
		stockItemHistory(id, { limit: LIMIT }).then(
			(page) => {
				if (!current) return;
				entries = page.items;
				failed = false;
			},
			() => {
				if (current) failed = entries === null;
			}
		);
		return () => {
			current = false;
		};
	});
</script>

<Section title={t.history}>
	{#if failed}
		<p class="muted">{t.historyFailed}</p>
	{:else if entries}
		<Timeline items={historyItems(entries)} now={serverClock.now()} emptyText={t.historyEmpty} />
	{/if}
</Section>

<style>
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
