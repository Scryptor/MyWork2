<!-- «История» of a position: the newest entries of its journal, «Показать ещё» for the older ones; the
     text of each is the server's summary. A new version of the position is a new entry: the newest page
     is asked again. -->
<script lang="ts">
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { historyItems } from '#lib/orders/history.js';
	import Button from '#lib/ui/Button.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Timeline from '#lib/ui/Timeline.svelte';
	import { createStockHistory } from './history.svelte.js';

	let { itemId, version }: { itemId: number; version: number } = $props();

	const t = ru.stock;
	const history = createStockHistory();

	$effect(() => {
		const id = itemId;
		void version;
		void history.refresh(id);
	});
</script>

<Section title={t.history}>
	{#if history.failed}
		<p class="muted">{t.historyFailed}</p>
	{:else}
		<Timeline
			items={historyItems(history.entries)}
			now={serverClock.now()}
			emptyText={t.historyEmpty}
		/>
		{#if history.moreFailed}
			<p class="muted" role="alert">{t.historyFailed}</p>
		{/if}
		{#if history.hasMore}
			<div>
				<Button size="sm" busy={history.loadingMore} onclick={() => void history.more()}
					>{history.moreFailed ? ru.common.retry : t.historyMore}</Button
				>
			</div>
		{/if}
	{/if}
</Section>

<style>
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
