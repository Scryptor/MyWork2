<!-- The unlock pattern of a device on a 3 × 3 grid (plan 8.2): the points are pressed in their order, a
     step over a point takes it too. The chosen points show their place in the pattern and a line joins
     them; under the grid — the pattern as text and how many points are left. -->
<script lang="ts">
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';
	import {
		addPoint,
		PATTERN_MIN_POINTS,
		PATTERN_POINTS,
		patternReady,
		patternText
	} from '#lib/orders/pattern.js';
	import Button from './Button.svelte';

	let {
		value = $bindable(''),
		disabled = false,
		id
	}: {
		/** The points in their order: «1596»; empty — no pattern. */
		value?: string;
		disabled?: boolean;
		id?: string;
	} = $props();

	const t = ru.pattern;
	/** The centre of a point on the grid of the line, where a cell is 1 × 1. */
	const centre = (p: number) => `${((p - 1) % 3) + 0.5},${Math.floor((p - 1) / 3) + 0.5}`;
	const line = $derived([...value].map((c) => centre(Number(c))).join(' '));
	const ready = $derived(patternReady(value));
	const left = $derived(PATTERN_MIN_POINTS - value.length);
</script>

<div class="pattern" {id}>
	<div class="grid" role="group" aria-label={t.points}>
		<svg class="line" viewBox="0 0 3 3" aria-hidden="true">
			{#if value.length > 1}<polyline points={line} />{/if}
		</svg>
		{#each PATTERN_POINTS as p (p)}
			{@const place = value.indexOf(String(p)) + 1}
			<button
				type="button"
				class="point"
				class:on={place > 0}
				aria-label={t.point(p)}
				aria-pressed={place > 0}
				{disabled}
				onclick={() => (value = addPoint(value, p))}
			>
				<span class="dot">{place > 0 ? place : ''}</span>
			</button>
		{/each}
	</div>
	<p class="sequence num" class:start={value === ''}>
		{value === '' ? t.start : patternText(value)}
	</p>
	<p class="state" class:ready aria-live="polite">
		{#if ready}{t.ready}{:else if value !== ''}{t.more(countLabel(left, t.pointForms))}{/if}
	</p>
	<div class="actions">
		<Button
			size="sm"
			variant="ghost"
			icon={Undo2}
			disabled={disabled || value === ''}
			onclick={() => (value = value.slice(0, -1))}>{t.undo}</Button
		>
		<Button
			size="sm"
			variant="ghost"
			icon={RotateCcw}
			disabled={disabled || value === ''}
			onclick={() => (value = '')}>{t.reset}</Button
		>
	</div>
</div>

<style>
	.pattern {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		width: fit-content;
		max-width: 100%;
	}
	/* Equal cells without gaps: the centre of a point is the centre of its cell, where the line goes. A
	   cell is a finger wide on a touch screen: --control-lg grows there. */
	.grid {
		--cell: calc(var(--control-lg) + var(--space-4));
		position: relative;
		display: grid;
		grid-template-columns: repeat(3, var(--cell));
		grid-auto-rows: var(--cell);
	}
	.line {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: visible;
		pointer-events: none;
	}
	.line polyline {
		fill: none;
		stroke: var(--accent);
		stroke-width: var(--space-1);
		stroke-linecap: round;
		stroke-linejoin: round;
		vector-effect: non-scaling-stroke;
	}
	.point {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--on-accent);
		font: inherit;
		cursor: pointer;
	}
	.point:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: -2px;
	}
	.point:disabled {
		cursor: not-allowed;
	}
	.dot {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		box-sizing: border-box;
		width: var(--space-3);
		height: var(--space-3);
		border-radius: var(--radius-full);
		background: var(--text-3);
		font-size: var(--text-sm);
		font-weight: 600;
		transition:
			width var(--duration-fast),
			height var(--duration-fast),
			background var(--duration-fast);
	}
	.point:hover:not(:disabled) .dot {
		background: var(--accent-hover);
	}
	.point.on .dot {
		width: var(--control-sm);
		height: var(--control-sm);
		background: var(--accent);
	}
	.point:disabled .dot {
		background: var(--text-disabled);
	}
	/* One line high with the pattern or without: the grid does not jump at the first point. */
	.sequence {
		min-height: calc(var(--text-lg) * var(--leading-normal));
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 500;
		text-align: center;
	}
	.sequence.start {
		color: var(--text-3);
		font-size: var(--text-md);
		font-weight: 400;
	}
	.state {
		min-height: calc(var(--text-sm) * var(--leading-normal));
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.state.ready {
		color: var(--success-text);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--space-2);
	}
</style>
