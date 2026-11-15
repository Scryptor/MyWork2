<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';

	let {
		text,
		side = 'top',
		children
	}: { text: string; side?: 'top' | 'bottom' | 'left' | 'right'; children: Snippet } = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<span {...props} class="trigger">{@render children()}</span>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Portal>
		<Tooltip.Content {side} sideOffset={6} class="mw-tooltip">{text}</Tooltip.Content>
	</Tooltip.Portal>
</Tooltip.Root>

<style>
	.trigger {
		display: inline-flex;
	}
	:global(.mw-tooltip) {
		z-index: var(--z-tooltip, 60);
		background: var(--text);
		color: var(--bg);
		font-size: var(--text-xs);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		box-shadow: var(--shadow-md);
		max-width: 20rem;
	}
</style>
