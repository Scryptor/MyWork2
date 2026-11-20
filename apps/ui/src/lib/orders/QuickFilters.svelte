<!-- Quick filters of the orders list (spec 6.1): click picks one, Ctrl/⌘+click adds a status.
     Statuses without orders wait in «Ещё»; a chosen one is shown among the chips. -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import FilterChips from '#lib/ui/FilterChips.svelte';
	import { moreFilterMenu, type QuickFilterItem } from './view.js';

	let {
		items,
		selected,
		onchange
	}: {
		items: { main: QuickFilterItem[]; more: QuickFilterItem[] };
		/** The pressed chips; none while another part of the view decides. */
		selected: string[];
		onchange: (chips: string[]) => void;
	} = $props();

	const shown = $derived.by(() => {
		const picked = items.more.filter((i) => selected.includes(i.id));
		if (picked.length === 0) return items.main;
		// Before «Выданные» and «Все»: among the statuses.
		const tail = items.main.filter((i) => i.id === 'issued' || i.id === 'all');
		const head = items.main.filter((i) => !tail.includes(i));
		return [...head, ...picked, ...tail];
	});
	const rest = $derived<MenuItem[]>(
		moreFilterMenu(items.more, selected).map((e) =>
			'heading' in e
				? e
				: {
						label: e.item.count === undefined ? e.item.label : `${e.item.label} · ${e.item.count}`,
						onselect: () => onchange([e.item.id])
					}
		)
	);
</script>

<div class="quick">
	<FilterChips
		items={shown}
		label={ru.orders.quickFilters}
		bind:selected={() => selected, (v) => onchange(v)}
	/>
	{#if rest.length > 0}
		<DropdownMenu items={rest} label={ru.orders.moreStatuses}>
			{#snippet trigger(props)}
				<!-- The name says what is in «Ещё»: the bottom bar of a phone has its own «Ещё». -->
				<Button
					{...props}
					variant="ghost"
					iconEnd={ChevronDown}
					aria-label={ru.orders.moreStatusesButton}>{ru.common.more}</Button
				>
			{/snippet}
		</DropdownMenu>
	{/if}
</div>

<style>
	.quick {
		display: flex;
		align-items: center;
		gap: var(--space-1-5);
		min-width: 0;
	}
	/* The chips shrink (and wrap, or scroll sideways on a phone); «Ещё» keeps its size. */
	.quick > :global(:first-child) {
		min-width: 0;
	}
	.quick > :global(:not(:first-child)) {
		flex: none;
	}
</style>
