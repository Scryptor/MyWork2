<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';
	import { groupDigits } from '#lib/format/digits.js';
	import { ru } from '#lib/i18n/ru.js';
	import IconButton from '../IconButton.svelte';

	/**
	 * Bulk actions for the selected rows; nothing is shown while nothing is selected. `locked`: an action
	 * runs, the selection cannot be cleared until it ends.
	 */
	let {
		count,
		locked = false,
		onclear,
		children
	}: { count: number; locked?: boolean; onclear: () => void; children?: Snippet } = $props();

	// Thousands separated by a narrow no-break space, as in sums: «Выбрано 2 500».
	const shown = $derived(groupDigits(String(count)));
</script>

{#if count > 0}
	<div class="bar" role="region" aria-label={ru.table.selection}>
		<span class="count" aria-live="polite">{ru.table.selected} {shown}</span>
		{#if children}<span class="actions">{@render children()}</span>{/if}
		<IconButton
			label={ru.table.clearSelection}
			icon={X}
			size="sm"
			disabled={locked}
			onclick={onclear}
		/>
	</div>
{/if}

<style>
	.bar {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		max-width: 100%;
		box-sizing: border-box;
		padding: var(--space-1) var(--space-1) var(--space-1) var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-lg);
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow-md);
		font-size: var(--text-md);
	}
	.count {
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1);
		padding-left: var(--space-2);
		border-left: 1px solid var(--border);
	}
</style>
