<!-- A big choice of what to do, with a line of what it means (the first run: an empty base or a transfer). -->
<script lang="ts">
	import type { Component } from 'svelte';

	let {
		icon,
		title,
		description,
		onclick
	}: { icon?: Component; title: string; description?: string; onclick: () => void } = $props();

	const id = $props.id();
	const Icon = $derived(icon);
</script>

<button
	type="button"
	class="option"
	aria-labelledby="{id}-title"
	aria-describedby={description ? `${id}-desc` : undefined}
	{onclick}
>
	{#if Icon}<span class="icon"><Icon size={20} aria-hidden="true" /></span>{/if}
	<span class="body">
		<span class="title" id="{id}-title">{title}</span>
		{#if description}<span class="desc" id="{id}-desc">{description}</span>{/if}
	</span>
</button>

<style>
	.option {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		box-sizing: border-box;
		width: 100%;
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-lg);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition:
			background var(--duration-fast),
			border-color var(--duration-fast);
	}
	.option:hover {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.icon {
		display: inline-flex;
		flex: none;
		padding-top: var(--space-0-5);
		color: var(--accent-text);
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
		min-width: 0;
	}
	.title {
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.desc {
		color: var(--text-2);
		font-size: var(--text-md);
	}
</style>
