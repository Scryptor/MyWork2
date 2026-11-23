<!-- The properties of the label of the copy («Экземпляр сервиса» / «клиента»): the alignment. -->
<script lang="ts">
	import type { Align } from '$api/Align';
	import type { BlockBody } from '$api/BlockBody';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';

	type Body = Extract<BlockBody, { type: 'copy_label' }>;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'copy_label' ? block.body : null);
	const key = $derived(path.join('.'));

	/** A change of the block; typed fields pass their key, so their keys make one step. */
	function set(patch: Partial<Omit<Body, 'type'>>, field?: string) {
		if (!block || !body) return;
		const next = { ...block, body: { ...body, ...patch } };
		editor.edit(
			replaceBlock(editor.template, path, next),
			field === undefined ? undefined : `${key}.${field}`
		);
	}

	const ALIGNS: Align[] = ['left', 'center', 'right'];
	const aligns = ALIGNS.map((value) => ({ value, label: t.aligns[value] }));
</script>

{#if body}
	<SegmentedControl
		label={t.align}
		options={aligns}
		bind:value={() => body.align, (v) => set({ align: v })}
	/>
{/if}
