<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import Kbd from './Kbd.svelte';
	import Spinner from './Spinner.svelte';

	let {
		variant = 'secondary',
		size = 'md',
		type = 'button',
		busy = false,
		disabled = false,
		icon,
		shortcut,
		href,
		onclick,
		children,
		class: className = ''
	}: {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md' | 'lg';
		type?: 'button' | 'submit';
		busy?: boolean;
		disabled?: boolean;
		icon?: Component;
		shortcut?: string;
		href?: string;
		onclick?: (e: MouseEvent) => void;
		children?: Snippet;
		class?: string;
	} = $props();

	const Icon = $derived(icon);

	function handle(e: MouseEvent) {
		if (busy || disabled) {
			e.preventDefault();
			return;
		}
		onclick?.(e);
	}
</script>

{#snippet content()}
	<span class="inner" class:hidden={busy}>
		{#if Icon}<Icon size={16} aria-hidden="true" />{/if}
		{#if children}<span>{@render children()}</span>{/if}
		{#if shortcut}<Kbd {shortcut} />{/if}
	</span>
	{#if busy}<span class="spin"><Spinner size="sm" /></span>{/if}
{/snippet}

{#if href && !disabled}
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- callers pass an already resolved href -->
	<a class="btn {variant} {size} {className}" {href} onclick={handle}>{@render content()}</a>
{:else}
	<button
		class="btn {variant} {size} {className}"
		{type}
		{disabled}
		aria-busy={busy ? 'true' : undefined}
		onclick={handle}
	>
		{@render content()}
	</button>
{/if}

<style>
	.btn {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		box-sizing: border-box;
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		font: inherit;
		font-size: var(--text-md);
		font-weight: 500;
		text-decoration: none;
		cursor: pointer;
		white-space: nowrap;
		transition:
			background var(--duration-fast),
			border-color var(--duration-fast);
	}
	.inner {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.inner.hidden {
		visibility: hidden;
	}
	.spin {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.sm {
		height: var(--control-sm);
		padding: 0 var(--space-2);
	}
	.md {
		height: var(--control-md);
		padding: 0 var(--space-3);
	}
	.lg {
		height: var(--control-lg);
		padding: 0 var(--space-4);
	}
	@media (pointer: coarse) {
		.btn {
			min-height: 2.75rem;
		}
	}
	.primary {
		background: var(--accent);
		color: var(--on-accent);
	}
	.primary:hover:not(:disabled) {
		background: var(--accent-hover);
	}
	.primary:active:not(:disabled) {
		background: var(--accent-active);
	}
	.danger {
		background: var(--danger);
		color: var(--on-danger);
	}
	.danger:hover:not(:disabled) {
		background: var(--danger-hover);
	}
	.secondary {
		background: var(--surface);
		color: var(--text);
		border-color: var(--border-strong);
	}
	.secondary:hover:not(:disabled) {
		background: var(--surface-3);
	}
	.ghost {
		background: transparent;
		color: var(--text);
	}
	.ghost:hover:not(:disabled) {
		background: var(--surface-3);
	}
	.btn:disabled {
		color: var(--text-disabled);
		cursor: not-allowed;
	}
	.primary:disabled,
	.danger:disabled {
		background: var(--surface-3);
	}
	.btn[aria-busy='true'] {
		cursor: progress;
	}
</style>
