<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		title,
		actions,
		region = false,
		children
	}: {
		title: string;
		actions?: Snippet;
		/** A named region (landmark) of the page: for the blocks one jumps between, as on the order card. */
		region?: boolean;
		children: Snippet;
	} = $props();

	const id = $props.id();
</script>

<section class="section" aria-labelledby={region ? id : undefined}>
	<header>
		<h3 class="label-caps" {id}>{title}</h3>
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
