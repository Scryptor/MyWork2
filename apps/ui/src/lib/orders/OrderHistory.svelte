<!-- «История» of the panel: the three newest entries of the order's journal, «все» leads to the card. -->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { AuditEntry } from '$api/AuditEntry';
	import { orderHistory } from '#lib/api/orders.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Section from '#lib/ui/Section.svelte';
	import Timeline from '#lib/ui/Timeline.svelte';
	import { historyItems } from './history.js';

	const LIMIT = 3;

	let {
		orderId,
		version,
		onall
	}: {
		orderId: number;
		/** A new version is a new entry: the list is asked again. */
		version: number;
		/** «все»: the full card with the whole history. */
		onall: (e: MouseEvent) => void;
	} = $props();

	let entries = $state.raw<AuditEntry[] | null>(null);
	let failed = $state(false);

	$effect(() => {
		const id = orderId;
		void version;
		let current = true;
		orderHistory(id, { limit: LIMIT }).then(
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

<Section title={ru.order.history}>
	{#snippet actions()}
		<a class="all" href={resolve(`orders/${orderId}`)} onclick={onall}>{ru.order.allHistory}</a>
	{/snippet}
	{#if failed}
		<p class="muted">{ru.order.historyFailed}</p>
	{:else if entries}
		<Timeline
			items={historyItems(entries)}
			now={serverClock.now()}
			emptyText={ru.order.historyEmpty}
		/>
	{/if}
</Section>

<style>
	.all {
		color: var(--accent);
		font-size: var(--text-sm);
	}
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
