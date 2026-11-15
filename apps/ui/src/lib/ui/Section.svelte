<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, actions, children }: { title: string; actions?: Snippet; children: Snippet } =
		$props();
</script>

<section class="section">
	<header>
		<h3 class="label-caps">{title}</h3>
		{#if actions}<div class="actions">{@render actions()}</div>{/if}
	</header>
	{@render children()}
</section>

<style>
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}
	h3 {
		margin: 0;
	}
	.actions {
		display: flex;
		gap: var(--space-2);
	}
</style>
