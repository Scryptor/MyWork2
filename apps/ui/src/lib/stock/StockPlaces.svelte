<!-- «Где лежит» (plan 8.6): the cells a position lies in and the unplaced rest. The actions go through the
     editor of the position: what was typed is saved first, the answer is taken by the editor. -->
<script lang="ts">
	import type { StockPlace } from '$api/StockPlace';
	import type { StockRack } from '$api/StockRack';
	import { errorMessage } from '#lib/api/client.js';
	import { getStockItem, moveStock, putStock, unplaceStock } from '#lib/api/stock.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Section from '#lib/ui/Section.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import type { StockEditor } from './editor.svelte.js';
	import PlaceDialog from './PlaceDialog.svelte';
	import { cellOf } from './places.js';

	let {
		editor,
		racks,
		canEdit
	}: {
		editor: StockEditor;
		/** The racks; null while they are not known (the block shows no hint then). */
		racks: StockRack[] | null;
		canEdit: boolean;
	} = $props();

	const t = ru.stock.places;
	const item = $derived(editor.item);
	let putting = $state(false);
	let moving = $state<StockPlace | null>(null);

	const hasRacks = $derived((racks?.length ?? 0) > 0);

	async function unplace(p: StockPlace) {
		try {
			await editor.run(() => unplaceStock(item.id, cellOf(p)));
		} catch (e) {
			toast.show({ message: t.failed(errorMessage(e)), tone: 'danger' });
		}
	}
</script>

<Section title={t.title}>
	{#if item.places.length > 0 || item.unplaced > 0}
		<ul>
			{#each item.places as p (p.code)}
				<li>
					<span class="num">{t.line(p.code, p.quantity)}</span>
					{#if canEdit}
						<span class="row-actions">
							{#if hasRacks}
								<Button
									size="sm"
									variant="ghost"
									aria-label={t.label(t.move, p.code)}
									onclick={() => (moving = p)}>{t.move}</Button
								>
							{/if}
							<Button
								size="sm"
								variant="ghost"
								aria-label={t.label(t.unplace, p.code)}
								onclick={() => void unplace(p)}>{t.unplace}</Button
							>
						</span>
					{/if}
				</li>
			{/each}
			{#if item.unplaced > 0}
				<li>
					<span class="num">{t.unplaced(item.unplaced)}</span>
					{#if canEdit && hasRacks}
						<Button size="sm" variant="ghost" onclick={() => (putting = true)}>{t.put}</Button>
					{/if}
				</li>
			{/if}
		</ul>
	{:else}
		<p class="hint">{t.nothing}</p>
	{/if}
	{#if racks !== null && !hasRacks}<p class="hint">{t.noRacks}</p>{/if}
</Section>

{#if canEdit && hasRacks && racks}
	<PlaceDialog
		bind:open={putting}
		mode="put"
		{racks}
		{item}
		onsubmit={async (_id, ref, quantity) => {
			await editor.run(() => putStock(item.id, { place: ref, quantity }));
		}}
		reload={async (id) => {
			const fresh = await getStockItem(id);
			editor.receive(fresh);
			return fresh;
		}}
	/>
	<PlaceDialog
		bind:open={() => moving !== null, (v) => !v && (moving = null)}
		mode="move"
		{racks}
		{item}
		from={moving ?? undefined}
		onsubmit={async (_id, to, quantity) => {
			const from = moving;
			if (!from) return;
			await editor.run(() => moveStock(item.id, { from: cellOf(from), to, quantity }));
		}}
		reload={async (id) => {
			const fresh = await getStockItem(id);
			editor.receive(fresh);
			return fresh;
		}}
	/>
{/if}

<style>
	ul {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}
	.row-actions {
		display: inline-flex;
		gap: var(--space-1);
	}
	.hint {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
