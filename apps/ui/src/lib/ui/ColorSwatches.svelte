<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import { STATUS_HUES, type StatusHue } from '#lib/design/status-colors.js';
	import { ru } from '#lib/i18n/ru.js';

	let {
		value,
		onchange,
		label,
		disabled = false
	}: {
		value: StatusHue;
		onchange: (hue: StatusHue) => void;
		label: string;
		disabled?: boolean;
	} = $props();

	const name = $props.id();
</script>

<div class="swatches" role="radiogroup" aria-label={label}>
	{#each STATUS_HUES as hue (hue)}
		<label class="swatch st-{hue}" class:on={value === hue} class:disabled>
			<input
				type="radio"
				{name}
				value={hue}
				{disabled}
				checked={value === hue}
				aria-label={ru.hues[hue]}
				onchange={() => onchange(hue)}
			/>
			<span class="dot" aria-hidden="true">
				{#if value === hue}<Check size={14} />{/if}
			</span>
		</label>
	{/each}
</div>

<style>
	.swatches {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.swatch {
		position: relative;
		display: inline-flex;
		cursor: pointer;
	}
	.swatch.disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.dot {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.75rem;
		height: 1.75rem;
		box-sizing: border-box;
		border: 2px solid transparent;
		border-radius: var(--radius-full);
		background: var(--st-dot);
		color: var(--st-bg);
	}
	.on .dot {
		border-color: var(--surface);
		box-shadow: 0 0 0 2px var(--st-dot);
	}
	@media (pointer: coarse) {
		.dot {
			width: 2.75rem;
			height: 2.75rem;
		}
	}
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: inherit;
	}
	.swatch:has(input:focus-visible) .dot {
		outline: 2px solid var(--focus);
		outline-offset: 3px;
	}
</style>
