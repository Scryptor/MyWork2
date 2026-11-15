<script lang="ts" generics="T extends string | number">
	import type { Component } from 'svelte';

	let {
		value = $bindable(),
		options,
		label,
		size = 'md'
	}: {
		value: T;
		options: { value: T; label: string; icon?: Component }[];
		label: string;
		size?: 'sm' | 'md';
	} = $props();

	const name = $props.id();
</script>

<div class="seg {size}" role="radiogroup" aria-label={label}>
	{#each options as o (o.value)}
		{@const Icon = o.icon}
		<label class:on={value === o.value}>
			<input
				type="radio"
				{name}
				value={o.value}
				checked={value === o.value}
				onchange={() => (value = o.value)}
			/>
			{#if Icon}<Icon size={14} aria-hidden="true" />{/if}
			<span>{o.label}</span>
		</label>
	{/each}
</div>

<style>
	.seg {
		display: inline-flex;
		padding: 2px;
		gap: 2px;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}
	label {
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
		height: calc(var(--control-md) - 6px);
		padding: 0 var(--space-3);
		border-radius: var(--radius-sm);
		font-size: var(--text-md);
		color: var(--text-2);
		cursor: pointer;
		white-space: nowrap;
	}
	.sm label {
		height: calc(var(--control-sm) - 6px);
		padding: 0 var(--space-2);
	}
	@media (pointer: coarse) {
		label {
			min-height: 2.5rem;
		}
	}
	label:hover:not(.on) {
		background: var(--surface-3);
	}
	label.on {
		background: var(--surface);
		color: var(--text);
		font-weight: 500;
		box-shadow: var(--shadow-xs);
	}
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}
	label:has(input:focus-visible) {
		outline: 2px solid var(--focus);
		outline-offset: 1px;
	}
</style>
