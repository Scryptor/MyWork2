<!-- Checkbox of the table: looks like Checkbox, but has no visible label, can stay out of the Tab
     order and passes the click (with Shift) to the table. The table owns the state. -->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import Minus from '@lucide/svelte/icons/minus';

	let {
		checked,
		indeterminate = false,
		label,
		tabindex,
		disabled = false,
		onclick
	}: {
		checked: boolean;
		indeterminate?: boolean;
		label: string;
		tabindex?: number;
		/** The selection cannot change now (a bulk action runs). */
		disabled?: boolean;
		onclick: (e: MouseEvent) => void;
	} = $props();

	let el = $state<HTMLInputElement | null>(null);
	$effect(() => {
		if (el) el.indeterminate = indeterminate;
	});

	function handle(e: MouseEvent) {
		onclick(e);
		// The click has already flipped the box; show what the table decided (e.g. Shift+click on a
		// selected row keeps it selected).
		if (el) {
			el.checked = checked;
			el.indeterminate = indeterminate;
		}
	}
</script>

<span class="box">
	<input
		bind:this={el}
		type="checkbox"
		{checked}
		{tabindex}
		{disabled}
		aria-label={label}
		onclick={handle}
	/>
	<span class="mark" aria-hidden="true">
		{#if indeterminate}<Minus size={14} />{:else if checked}<Check size={14} />{/if}
	</span>
</span>

<style>
	.box {
		position: relative;
		display: inline-flex;
		flex: none;
		width: 1.125rem;
		height: 1.125rem;
	}
	/* The hit area is larger than the box; the table cell clips it. */
	input {
		position: absolute;
		inset: calc(-1 * var(--space-2));
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}
	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		box-sizing: border-box;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-xs);
		background: var(--surface);
		color: var(--on-accent);
		pointer-events: none;
	}
	input:checked + .mark,
	input:indeterminate + .mark {
		background: var(--accent);
		border-color: var(--accent);
	}
	input:disabled {
		cursor: not-allowed;
	}
	input:disabled + .mark {
		opacity: 0.5;
	}
	input:focus-visible + .mark {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
</style>
