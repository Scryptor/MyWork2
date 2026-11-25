<script lang="ts">
	import { Popover } from 'bits-ui';
	import { untrack, type Snippet } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import './popup.css';

	let {
		open = $bindable(false),
		trigger,
		children,
		side = 'bottom',
		align = 'start',
		width,
		label
	}: {
		open?: boolean;
		/** Spread the props onto the trigger element: `<button {...props}>`. */
		trigger: Snippet<[Record<string, unknown>]>;
		children: Snippet;
		side?: 'top' | 'bottom' | 'left' | 'right';
		align?: 'start' | 'center' | 'end';
		width?: string;
		/** A name for the popover, such as a form of filters: it becomes a named dialog. */
		label?: string;
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
			role={label ? 'dialog' : undefined}
			aria-label={label}
			style={width ? `width: ${width}` : undefined}
		>
			{@render children()}
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>
