<!-- A pane of `Tabs` with its own markup: a tab panel (named by its tab, hidden unless active), or `bare`:
     no tab semantics, always shown and laid out as if the pane were not there (`display: contents`). -->
<script lang="ts">
	import { Tabs } from 'bits-ui';
	import type { Snippet } from 'svelte';

	let {
		value,
		bare = false,
		children
	}: { value: string; bare?: boolean; children: Snippet } = $props();
</script>

<Tabs.Content {value}>
	{#snippet child({ props })}
		<!-- One element in both modes, so the content is not rebuilt when `bare` changes. -->
		<div {...bare ? { 'data-value': value } : props} class:mw-pane-bare={bare}>
			{@render children()}
		</div>
	{/snippet}
</Tabs.Content>

<style>
	.mw-pane-bare {
		display: contents;
	}
</style>
