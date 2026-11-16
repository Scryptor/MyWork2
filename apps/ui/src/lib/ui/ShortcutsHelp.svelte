<script lang="ts">
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Dialog from './Dialog.svelte';
	import Kbd from './Kbd.svelte';

	let {
		open = $bindable(false),
		groups
	}: {
		open: boolean;
		groups: { title: string; items: { label: string; shortcut: string }[] }[];
	} = $props();

	$effect(() => registerHotkey('?', () => (open = true)));
</script>

<Dialog bind:open title={ru.shortcuts.title}>
	{#each groups as group (group.title)}
		<section class="group">
			<h3 class="label-caps">{group.title}</h3>
			<dl>
				{#each group.items as item (item.label)}
					<div class="row">
						<dt>{item.label}</dt>
						<dd><Kbd shortcut={item.shortcut} /></dd>
					</div>
				{/each}
			</dl>
		</section>
	{/each}
</Dialog>

<style>
	.group + .group {
		margin-top: var(--space-4);
	}
	h3 {
		margin: 0 0 var(--space-1);
	}
	dl {
		margin: 0;
	}
	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-1-5) 0;
		border-bottom: 1px solid var(--border);
	}
	dt {
		color: var(--text);
	}
	dd {
		margin: 0;
	}
</style>
