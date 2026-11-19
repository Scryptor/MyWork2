<script lang="ts">
	import type { Component } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import { mergeTriggerProps } from './merge-props.js';
	import Tooltip from './Tooltip.svelte';

	let {
		label,
		icon,
		size = 'md',
		variant = 'ghost',
		disabled = false,
		pressed,
		onclick,
		...rest
	}: Omit<HTMLButtonAttributes, 'children' | 'class' | 'type' | 'onclick' | 'disabled'> & {
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
			{...mergeTriggerProps(triggerProps, { ...rest, onclick })}
			type="button"
			class="ib {variant} {size}"
			aria-label={label}
			aria-pressed={pressed}
			{disabled}
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
			min-width: var(--control-md);
			min-height: var(--control-md);
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
