<!-- The colour of a status: a round button in that colour; it opens the twelve hues to choose from. -->
<script lang="ts">
	import { statusHue } from '#lib/design/status-colors.js';
	import { ru } from '#lib/i18n/ru.js';
	import ColorSwatches from '#lib/ui/ColorSwatches.svelte';
	import Popover from '#lib/ui/Popover.svelte';

	let {
		color,
		name,
		disabled = false,
		onchange
	}: {
		color: string;
		/** The status the colour belongs to, for the name of the button. */
		name: string;
		disabled?: boolean;
		onchange: (hue: string) => void;
	} = $props();

	let open = $state(false);
	const hue = $derived(statusHue(color));
</script>

<Popover bind:open label={ru.statuses.color}>
	{#snippet trigger(props)}
		<button
			{...props}
			type="button"
			class="dot st-{hue}"
			{disabled}
			aria-label={ru.statuses.colorOf(name, ru.hues[hue].toLowerCase())}
		></button>
	{/snippet}
	<ColorSwatches
		value={hue}
		label={ru.statuses.color}
		{disabled}
		selectOnArrow={false}
		onchange={(h) => {
			open = false;
			onchange(h);
		}}
	/>
</Popover>

<style>
	.dot {
		flex: none;
		width: 1.5rem;
		height: 1.5rem;
		padding: 0;
		box-sizing: border-box;
		border: 2px solid var(--surface);
		border-radius: var(--radius-full);
		background: var(--st-dot);
		box-shadow: 0 0 0 1px var(--border-strong);
		cursor: pointer;
	}
	.dot:disabled {
		cursor: not-allowed;
		opacity: 0.6;
	}
	.dot:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
	@media (pointer: coarse) {
		.dot {
			width: 2.75rem;
			height: 2.75rem;
		}
	}
</style>
