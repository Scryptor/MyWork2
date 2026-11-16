<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, search, actions }: { title: string; search?: Snippet; actions?: Snippet } = $props();
</script>

<header class="topbar">
	<strong class="title">{title}</strong>
	<div class="search">{@render search?.()}</div>
	<div class="actions">{@render actions?.()}</div>
</header>

<style>
	.topbar {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		box-sizing: border-box;
		height: var(--topbar-height);
		padding: 0 var(--space-3);
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}
	.title {
		flex: none;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.search {
		flex: 1;
		display: flex;
		justify-content: center;
		min-width: 0;
	}
	.actions {
		flex: none;
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
</style>
