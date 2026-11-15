<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';

	let {
		text,
		side = 'top',
		children
	}: {
		text: string;
		side?: 'top' | 'bottom' | 'left' | 'right';
		children: Snippet<[Record<string, unknown>]>;
	} = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			{@render children(props)}
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Portal>
		<Tooltip.Content {side} sideOffset={6} class="mw-tooltip" role="tooltip">{text}</Tooltip.Content
		>
	</Tooltip.Portal>
</Tooltip.Root>

<style>
	:global(.mw-tooltip) {
		z-index: var(--z-tooltip);
		background: var(--text);
		color: var(--bg);
		font-size: var(--text-xs);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		box-shadow: var(--shadow-md);
		max-width: 20rem;
	}
</style>
