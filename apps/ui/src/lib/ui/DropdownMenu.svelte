<script lang="ts" module>
	import type { Component } from 'svelte';

	export type MenuItem =
		| {
				label: string;
				icon?: Component;
				shortcut?: string;
				onselect: () => void;
				disabled?: boolean;
				danger?: boolean;
		  }
		| { separator: true }
		/** Names the items after it, up to the next separator or heading: a group for screen readers. */
		| { heading: string };

	type Action = Exclude<MenuItem, { separator: true } | { heading: string }>;
	type Block = { separator: true } | { action: Action } | { heading: string; actions: Action[] };

	/** The flat list as the menu shows it: a heading with its items is one group. */
	function blocks(items: readonly MenuItem[]): Block[] {
		const out: Block[] = [];
		let group: { heading: string; actions: Action[] } | null = null;
		for (const item of items) {
			if ('separator' in item) {
				group = null;
				out.push(item);
			} else if ('heading' in item) {
				group = { heading: item.heading, actions: [] };
				out.push(group);
			} else if (group) group.actions.push(item);
			else out.push({ action: item });
		}
		return out;
	}
</script>

<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { untrack, type Snippet } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import Kbd from './Kbd.svelte';
	import './popup.css';

	let {
		trigger,
		items,
		label
	}: {
		/** Spread the props onto the trigger element: `<button {...props}>`. */
		trigger: Snippet<[Record<string, unknown>]>;
		items: MenuItem[];
		/** Accessible name of the menu. */
		label?: string;
	} = $props();

	let open = $state(false);

	$effect(() => {
		if (open) return untrack(() => layers.open('popup', () => (open = false)));
	});
</script>

{#snippet menuItem(item: Action)}
	{@const Icon = item.icon}
	<DropdownMenu.Item
		class="mw-option mw-menu-item"
		data-danger={item.danger ? '' : undefined}
		disabled={item.disabled}
		textValue={item.label}
		onSelect={item.onselect}
	>
		{#if Icon}<Icon size={18} aria-hidden="true" />{/if}
		<span class="mw-menu-label">{item.label}</span>
		{#if item.shortcut}<Kbd shortcut={item.shortcut} />{/if}
	</DropdownMenu.Item>
{/snippet}

<DropdownMenu.Root bind:open>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}{@render trigger(props)}{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content
			class="mw-popup mw-menu"
			sideOffset={4}
			align="start"
			collisionPadding={8}
			aria-label={label}
		>
			{#each blocks(items) as block, i (i)}
				{#if 'separator' in block}
					<DropdownMenu.Separator class="mw-menu-sep" />
				{:else if 'heading' in block}
					<DropdownMenu.Group>
						<DropdownMenu.GroupHeading class="mw-menu-heading"
							>{block.heading}</DropdownMenu.GroupHeading
						>
						{#each block.actions as action, j (j)}{@render menuItem(action)}{/each}
					</DropdownMenu.Group>
				{:else}
					{@render menuItem(block.action)}
				{/if}
			{/each}
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>

<style>
	:global(.mw-menu-item.mw-option) {
		justify-content: flex-start;
		gap: var(--space-2);
		font-weight: 500;
	}
	:global(.mw-menu-item[data-danger]) {
		color: var(--danger-text);
	}
	:global(.mw-menu-item[data-disabled]) {
		color: var(--text-disabled);
	}
	:global(.mw-menu-label) {
		flex: 1;
		min-width: 0;
	}
	:global(.mw-menu-heading) {
		padding: var(--space-1-5) var(--space-2) var(--space-1);
		font-size: var(--text-sm);
		color: var(--text-3);
	}
	:global(.mw-menu-sep) {
		height: 1px;
		margin: var(--space-1) 0;
		background: var(--border);
	}
</style>
