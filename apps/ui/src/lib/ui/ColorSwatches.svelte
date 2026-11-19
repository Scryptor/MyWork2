<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import { STATUS_HUES, type StatusHue } from '#lib/design/status-colors.js';
	import { ru } from '#lib/i18n/ru.js';

	let {
		value,
		onchange,
		label,
		disabled = false,
		selectOnArrow = true
	}: {
		value: StatusHue;
		onchange: (hue: StatusHue) => void;
		label: string;
		disabled?: boolean;
		/** `false`: arrows only move the focus; Enter, Space or a click chooses (a palette in a popover that
		 *  closes on a choice). */
		selectOnArrow?: boolean;
	} = $props();

	const name = $props.id();

	const ARROWS = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'];

	function onkeydown(e: KeyboardEvent) {
		const input = e.target;
		if (selectOnArrow || !(input instanceof HTMLInputElement) || input.type !== 'radio') return;
		if (e.key === 'Enter') {
			e.preventDefault();
			if (!input.disabled) input.click();
			return;
		}
		if (!ARROWS.includes(e.key)) return;
		e.preventDefault();
		const radios = [
			...(input
				.closest('[role="radiogroup"]')
				?.querySelectorAll<HTMLInputElement>('input[type="radio"]:not(:disabled)') ?? [])
		];
		const at = radios.indexOf(input);
		const step = e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 1;
		radios[(at + step + radios.length) % radios.length]?.focus();
	}

	function sync(changed: HTMLInputElement) {
		const group = changed.closest('[role="radiogroup"]');
		for (const input of group?.querySelectorAll<HTMLInputElement>('input[type="radio"]') ?? []) {
			input.checked = input.value === value;
		}
	}
</script>

<div
	class="swatches"
	role="radiogroup"
	aria-label={label}
	aria-disabled={disabled ? 'true' : undefined}
	{onkeydown}
>
	{#each STATUS_HUES as hue (hue)}
		<label class="swatch st-{hue}" class:on={value === hue} class:disabled>
			<input
				type="radio"
				{name}
				value={hue}
				{disabled}
				checked={value === hue}
				aria-label={ru.hues[hue]}
				onchange={(e) => {
					onchange(hue);
					// Controlled: the radio that was clicked shows `value`, which the parent may have kept.
					sync(e.currentTarget);
				}}
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
