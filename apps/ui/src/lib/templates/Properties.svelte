<!-- The properties of what is chosen in the list of blocks: the page, or a block with «Экземпляры». The
     fields start anew for every item chosen (`#key`): what was typed into one does not stay for another. -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from './editor.svelte.js';
	import { blockAt } from './ops.js';
	import CopiesField from './props/CopiesField.svelte';
	import PageProps from './props/PageProps.svelte';
	import TextProps from './props/TextProps.svelte';

	let { editor }: { editor: TemplateEditor } = $props();

	const t = ru.templates;
	const selected = $derived(editor.selected);
	const block = $derived(selected === 'page' ? undefined : blockAt(editor.template, selected));
	const key = $derived(selected === 'page' ? 'page' : selected.join('.'));
</script>

<div class="properties">
	{#key key}
		{#if selected === 'page'}
			<h2 class="title">{t.page}</h2>
			<PageProps {editor} />
		{:else if block}
			<h2 class="title">{t.blockTypes[block.body.type]}</h2>
			{#if block.body.type === 'text'}
				<TextProps {editor} path={selected} />
			{:else}
				<p class="note">{t.noProperties}</p>
			{/if}
			<CopiesField {editor} path={selected} />
		{/if}
	{/key}
</div>

<style>
	.properties {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.title {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.note {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
