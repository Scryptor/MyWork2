<!-- «Экземпляры» of a block: in both copies of a document, or only in the service's or the client's.
     A document of one copy (a sticker) has nothing to choose. -->
<script lang="ts">
	import type { CopyFilter } from '$api/CopyFilter';
	import { ru } from '#lib/i18n/ru.js';
	import Select from '#lib/ui/Select.svelte';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const FILTERS: CopyFilter[] = ['both', 'service', 'client'];
	const items = FILTERS.map((value) => ({ value, label: t.copyFilters[value] }));
	const block = $derived(blockAt(editor.template, path));

	function choose(copies: CopyFilter | null) {
		if (!block || copies === null) return;
		editor.edit(replaceBlock(editor.template, path, { ...block, copies }));
	}
</script>

{#if block && editor.template.layout !== 'one_per_page'}
	<Select label={t.copies} {items} bind:value={() => block.copies, choose} />
{/if}
