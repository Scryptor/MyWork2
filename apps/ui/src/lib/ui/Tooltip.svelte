<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';

	let {
		text,
		side = 'top',
		mono = false,
		children
	}: {
		text: string;
		side?: 'top' | 'bottom' | 'left' | 'right';
		/** The text is code: monospaced. */
		mono?: boolean;
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
		<!-- Esc closes the tooltip and the panel under it at once. -->
		<Tooltip.Content
			{side}
			sideOffset={6}
			class={mono ? 'mw-tooltip mono' : 'mw-tooltip'}
			role="tooltip"
			onEscapeKeydown={(e) => layers.escapeFromTooltip(e)}>{text}</Tooltip.Content
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
	:global(.mw-tooltip.mono) {
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}
</style>
