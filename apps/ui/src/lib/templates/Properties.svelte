<!-- The properties of what is chosen in the list of blocks: the page, or a block with «Экземпляры». The
     fields start anew for every item chosen (`#key`): what was typed into one does not stay for another. -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from './editor.svelte.js';
	import { blockAt } from './ops.js';
	import BarcodeProps from './props/BarcodeProps.svelte';
	import ColumnsProps from './props/ColumnsProps.svelte';
	import CopyLabelProps from './props/CopyLabelProps.svelte';
	import CopiesField from './props/CopiesField.svelte';
	import FieldsProps from './props/FieldsProps.svelte';
	import LogoProps from './props/LogoProps.svelte';
	import PageProps from './props/PageProps.svelte';
	import QrProps from './props/QrProps.svelte';
	import SignaturesProps from './props/SignaturesProps.svelte';
	import SpacerProps from './props/SpacerProps.svelte';
	import TextProps from './props/TextProps.svelte';
	import TypstCodeProps from './props/TypstCodeProps.svelte';

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
			{:else if block.body.type === 'fields'}
				<FieldsProps {editor} path={selected} />
			{:else if block.body.type === 'barcode'}
				<BarcodeProps {editor} path={selected} />
			{:else if block.body.type === 'logo'}
				<LogoProps {editor} path={selected} />
			{:else if block.body.type === 'qr'}
				<QrProps {editor} path={selected} />
			{:else if block.body.type === 'signatures'}
				<SignaturesProps {editor} path={selected} />
			{:else if block.body.type === 'spacer'}
				<SpacerProps {editor} path={selected} />
			{:else if block.body.type === 'columns'}
				<ColumnsProps {editor} path={selected} />
			{:else if block.body.type === 'copy_label'}
				<CopyLabelProps {editor} path={selected} />
			{:else if block.body.type === 'typst_code'}
				<TypstCodeProps {editor} path={selected} />
			{/if}
			{#if block.body.type !== 'typst_code'}
				<CopiesField {editor} path={selected} />
			{/if}
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
</style>
