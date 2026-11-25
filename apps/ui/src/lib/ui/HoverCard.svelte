<!-- A card that shows more about its trigger while the pointer rests on it or keyboard focus is on it:
     a preview, read-only. Touch does not open it: a screen without hover shows what matters elsewhere. -->
<script lang="ts">
	import { LinkPreview } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import './popup.css';

	let {
		trigger,
		children,
		openDelay = 400,
		side = 'right'
	}: {
		/** Spread the props onto the trigger element: `<button {...props}>`. */
		trigger: Snippet<[Record<string, unknown>]>;
		children: Snippet;
		/** Milliseconds the pointer rests on the trigger before the card opens. */
		openDelay?: number;
		side?: 'top' | 'right' | 'bottom' | 'left';
	} = $props();
</script>

<!-- A press opens what the trigger opens (an order's panel); the panel covers the trigger, the pointer
     leaves it, and a card still waiting for its pause does not come (e2e: clients.spec.ts). -->
<LinkPreview.Root {openDelay} closeDelay={150}>
	<LinkPreview.Trigger>
		{#snippet child({ props })}{@render trigger(props)}{/snippet}
	</LinkPreview.Trigger>
	<LinkPreview.Portal>
		<!-- Esc closes the card and the panel under it at once, as for a tooltip. -->
		<LinkPreview.Content
			class="mw-popover mw-hover-card"
			{side}
			align="start"
			sideOffset={6}
			collisionPadding={8}
			onEscapeKeydown={(e) => layers.escapeFromTooltip(e)}
		>
			{@render children()}
		</LinkPreview.Content>
	</LinkPreview.Portal>
</LinkPreview.Root>

<style>
	/* Narrower than a popover, as wide as a tooltip; both classes, whichever CSS comes first. */
	:global(.mw-popover.mw-hover-card) {
		max-width: min(20rem, calc(100vw - 2 * var(--space-4)));
	}
</style>
