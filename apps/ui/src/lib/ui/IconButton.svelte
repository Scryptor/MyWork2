<script lang="ts">
	import type { Component } from 'svelte';
	import Tooltip from './Tooltip.svelte';

	let {
		label,
		icon,
		size = 'md',
		variant = 'ghost',
		disabled = false,
		pressed,
		onclick
	}: {
		label: string;
		icon: Component;
		size?: 'sm' | 'md';
		variant?: 'ghost' | 'secondary';
		disabled?: boolean;
		pressed?: boolean;
		onclick?: (e: MouseEvent) => void;
	} = $props();

	const Icon = $derived(icon);
</script>

<Tooltip text={label}>
	{#snippet children(triggerProps)}
		<button
			{...triggerProps}
			type="button"
			class="ib {variant} {size}"
			aria-label={label}
			aria-pressed={pressed}
			{disabled}
			{onclick}
		>
			<Icon size={size === 'sm' ? 14 : 16} aria-hidden="true" />
		</button>
	{/snippet}
</Tooltip>

<style>
	.ib {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		box-sizing: border-box;
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		color: var(--text-2);
		background: transparent;
		cursor: pointer;
	}
	.sm {
		width: var(--control-sm);
		height: var(--control-sm);
	}
	.md {
		width: var(--control-md);
		height: var(--control-md);
	}
	@media (pointer: coarse) {
		.ib {
			min-width: 2.75rem;
			min-height: 2.75rem;
		}
	}
	.secondary {
		background: var(--surface);
		border-color: var(--border-strong);
	}
	.ib:hover:not(:disabled),
	.ib[aria-pressed='true'] {
		background: var(--surface-3);
		color: var(--text);
	}
	.ib:disabled {
		color: var(--text-disabled);
		cursor: not-allowed;
	}
</style>
