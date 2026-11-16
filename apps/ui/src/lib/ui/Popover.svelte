<script lang="ts">
	import { Popover } from 'bits-ui';
	import { untrack, type Snippet } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';

	let {
		open = $bindable(false),
		trigger,
		children,
		side = 'bottom',
		align = 'start',
		width
	}: {
		open?: boolean;
		/** Spread the props onto the trigger element: `<button {...props}>`. */
		trigger: Snippet<[Record<string, unknown>]>;
		children: Snippet;
		side?: 'top' | 'bottom' | 'left' | 'right';
		align?: 'start' | 'center' | 'end';
		width?: string;
	} = $props();

	$effect(() => {
		if (open) return untrack(() => layers.open('popup', () => (open = false)));
	});
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}{@render trigger(props)}{/snippet}
	</Popover.Trigger>
	<Popover.Portal>
		<Popover.Content
			class="mw-popover"
			{side}
			{align}
			sideOffset={6}
			collisionPadding={8}
			style={width ? `width: ${width}` : undefined}
		>
			{@render children()}
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>

<style>
	:global(.mw-popover) {
		z-index: var(--z-popover);
		box-sizing: border-box;
		max-width: calc(100vw - 2 * var(--space-4));
		padding: var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow-md);
		font-size: var(--text-md);
	}
</style>
