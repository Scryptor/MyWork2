<script lang="ts">
	import type { Snippet } from 'svelte';

	let { padding = 'md', children }: { padding?: 'none' | 'sm' | 'md'; children: Snippet } =
		$props();
</script>

<div class="card {padding}">{@render children()}</div>

<style>
	.card {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		box-shadow: var(--shadow-xs);
	}
	.sm {
		padding: var(--space-3);
	}
	.md {
		padding: var(--space-4);
	}
</style>
