<script lang="ts">
	import type { Snippet } from 'svelte';
	import HelpHint from './HelpHint.svelte';

	let {
		title,
		actions,
		help,
		region = false,
		children
	}: {
		title: string;
		actions?: Snippet;
		/** «Что это?»: an icon right of the heading that opens the explanation. */
		help?: string | readonly string[];
		/** A named region (landmark) of the page: for the blocks one jumps between, as on the order card. */
		region?: boolean;
		children: Snippet;
	} = $props();

	const id = $props.id();
</script>

<section class="section" aria-labelledby={region ? id : undefined}>
	<header>
		<div class="heading">
			<h3 class="label-caps" {id}>{title}</h3>
			{#if help}<HelpHint {title} text={help} />{/if}
		</div>
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
	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
	}
	h3 {
		margin: 0;
	}
	.actions {
		display: flex;
		gap: var(--space-2);
	}
</style>
