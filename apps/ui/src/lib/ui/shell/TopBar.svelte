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
	/* The search is in the middle of the window when there is room: the side columns are equal. When
	   there is not, they grow with their content and the search shrinks (min-width 0), never over them. */
	.topbar {
		display: grid;
		grid-template-columns: 1fr minmax(0, var(--topbar-search-max)) 1fr;
		align-items: center;
		column-gap: var(--space-3);
		box-sizing: border-box;
		height: var(--topbar-height);
		padding: 0 var(--space-3);
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}
	.title {
		justify-self: start;
		font-size: var(--text-lg);
		font-weight: 600;
		white-space: nowrap;
	}
	.search {
		display: flex;
		justify-content: center;
		min-width: 0;
	}
	.actions {
		justify-self: end;
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
</style>
