<script lang="ts">
	import type { Component, Snippet } from 'svelte';

	let {
		icon,
		title,
		description,
		actions
	}: { icon?: Component; title: string; description?: string; actions?: Snippet } = $props();

	const Icon = $derived(icon);
</script>

<div class="empty">
	{#if Icon}<Icon size={32} aria-hidden="true" />{/if}
	<p class="title">{title}</p>
	{#if description}<p class="desc">{description}</p>{/if}
	{#if actions}<div class="actions">{@render actions()}</div>{/if}
</div>

<style>
	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-8) var(--space-4);
		text-align: center;
		color: var(--text-3);
	}
	p {
		margin: 0;
	}
	.title {
		color: var(--text);
		font-weight: 600;
	}
	.desc {
		font-size: var(--text-sm);
	}
	.actions {
		display: flex;
		gap: var(--space-2);
		margin-top: var(--space-2);
	}
</style>
