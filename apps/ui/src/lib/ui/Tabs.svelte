<script lang="ts">
	import { Tabs } from 'bits-ui';
	import type { Snippet } from 'svelte';

	let {
		value = $bindable(),
		items,
		content,
		children,
		hideList = false,
		class: className
	}: {
		value: string;
		items: { value: string; label: string; count?: number }[];
		/** Without it only the tab list is drawn: the panes are the page's own (one markup for any width). */
		content?: Snippet<[string]>;
		/** Own panes (`TabPane`) inside the tabs: the page keeps one markup for any layout. */
		children?: Snippet;
		/** The tab list is not shown (every pane is, see `TabPane` `bare`). */
		hideList?: boolean;
		class?: string;
	} = $props();
</script>

<Tabs.Root bind:value class={className}>
	<Tabs.List class="mw-tabs-list" hidden={hideList || undefined}>
		{#each items as item (item.value)}
			<Tabs.Trigger value={item.value} class="mw-tab">
				{item.label}
				{#if item.count !== undefined}<span class="num mw-tab-count">{item.count}</span>{/if}
			</Tabs.Trigger>
		{/each}
	</Tabs.List>
	{@render children?.()}
	{#if content}
		{#each items as item (item.value)}
			<Tabs.Content value={item.value} class="mw-tab-panel">
				{@render content(item.value)}
			</Tabs.Content>
		{/each}
	{/if}
</Tabs.Root>

<style>
	:global(.mw-tabs-list) {
		display: flex;
		gap: var(--space-1);
		border-bottom: 1px solid var(--border);
		overflow-x: auto;
	}
	:global(.mw-tabs-list[hidden]) {
		display: none;
	}
	:global(.mw-tab) {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
		min-height: var(--control-lg);
		padding: 0 var(--space-3);
		border: 0;
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
		background: transparent;
		color: var(--text-2);
		font: inherit;
		white-space: nowrap;
		cursor: pointer;
	}
	:global(.mw-tab:hover) {
		color: var(--text);
	}
	:global(.mw-tab[data-state='active']) {
		border-bottom-color: var(--accent);
		color: var(--accent-text);
		font-weight: 600;
	}
	:global(.mw-tab-count) {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	:global(.mw-tab-panel) {
		padding-top: var(--space-3);
	}
	@media (pointer: coarse) {
		:global(.mw-tab) {
			min-height: var(--control-md);
		}
	}
</style>
