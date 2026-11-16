<script lang="ts">
	import { statusHue } from '#lib/design/status-colors.js';

	let {
		items,
		selected = $bindable(),
		label
	}: {
		items: { id: string; label: string; count?: number; color?: string }[];
		selected: string[];
		label: string;
	} = $props();

	/** Click selects only this chip; Ctrl/⌘+click adds or removes it. */
	function pick(id: string, e: MouseEvent) {
		if (e.ctrlKey || e.metaKey) {
			selected = selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
		} else {
			selected = [id];
		}
	}
</script>

<div class="chips" role="group" aria-label={label}>
	{#each items as item (item.id)}
		<button
			type="button"
			class="chip"
			aria-pressed={selected.includes(item.id)}
			onclick={(e) => pick(item.id, e)}
		>
			{#if item.color}
				<span class="dot" style="--dot: var(--st-{statusHue(item.color)}-dot)" aria-hidden="true"
				></span>
			{/if}
			<span>{item.label}</span>
			{#if item.count !== undefined}<span class="count num">{item.count}</span>{/if}
		</button>
	{/each}
</div>

<style>
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1-5);
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
		box-sizing: border-box;
		min-height: var(--control-md);
		padding: 0 var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface);
		color: var(--text-2);
		font: inherit;
		font-size: var(--text-md);
		white-space: nowrap;
		cursor: pointer;
	}
	.chip:hover {
		background: var(--surface-3);
		color: var(--text);
	}
	.chip[aria-pressed='true'] {
		border-color: var(--accent);
		background: var(--accent-soft);
		color: var(--accent-text);
		font-weight: 600;
	}
	.dot {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: var(--radius-full);
		background: var(--dot);
	}
	.count {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.chip[aria-pressed='true'] .count {
		color: inherit;
	}
	@media (pointer: coarse) {
		.chip {
			min-height: 2.75rem;
		}
		/* Phone: one row, scrolls sideways. */
		.chips {
			flex-wrap: nowrap;
			overflow-x: auto;
		}
		.chip {
			flex: none;
		}
	}
</style>
