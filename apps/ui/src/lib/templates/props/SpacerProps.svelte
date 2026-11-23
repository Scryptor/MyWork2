<!-- The properties of an empty space: its height in millimetres. -->
<script lang="ts">
	import type { BlockBody } from '$api/BlockBody';
	import NumberField from './NumberField.svelte';
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';

	type Body = Extract<BlockBody, { type: 'spacer' }>;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'spacer' ? block.body : null);
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
</script>

{#if body}
	<NumberField
		label={t.spacerMm}
		value={body.mm}
		min={0}
		max={200}
		onchange={(n) => n !== null && set({ mm: n }, 'mm')}
	/>
{/if}
