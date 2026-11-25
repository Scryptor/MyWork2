<!-- The properties of a text block: the text (typing in it is one step to undo per pause), its size (empty:
     the template's), «Жирный», the alignment (off for a numbered list: it is always on the left), «В одну
     строку», «В рамке» and «Нумерованный список» (off beside «В одну строку»). Every change is an edit of
     the model. -->
<script lang="ts">
	import type { Align } from '$api/Align';
	import type { BlockBody } from '$api/BlockBody';
	import { ru } from '#lib/i18n/ru.js';
	import { formatMm } from '#lib/print/settings.svelte.js';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import type { TemplateEditor } from '../editor.svelte.js';
	import InsertField from '../InsertField.svelte';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';
	import NumberField from './NumberField.svelte';

	type TextBody = Extract<BlockBody, { type: 'text' }>;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const ALIGNS: Align[] = ['left', 'center', 'right'];
	const aligns = ALIGNS.map((value) => ({ value, label: t.aligns[value] }));

	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'text' ? block.body : null);
	const key = $derived(path.join('.'));
	/**
	 * A numbered list (not in one line, where the numbering is off) is always left-aligned: the choice
	 * shows «Слева» and is off; the block keeps its own alignment for when the numbering goes.
	 */
	const listAligned = $derived(body !== null && body.numbered && !body.one_line);
	let field = $state<HTMLTextAreaElement | null>(null);

	/** A change of the text block; typed fields pass their key, so their keys make one step. */
	function set(patch: Partial<Omit<TextBody, 'type'>>, field?: string) {
		if (!block || !body) return;
		const next = { ...block, body: { ...body, ...patch } };
		editor.edit(
			replaceBlock(editor.template, path, next),
			field === undefined ? undefined : `${key}.${field}`
		);
	}
</script>

{#if body}
	<div class="props">
		<Textarea
			label={t.text}
			bind:ref={field}
			minRows={3}
			maxRows={14}
			bind:value={() => body.text, (v) => set({ text: v }, 'text')}
		/>
		<InsertField target={field} value={body.text} onchange={(v) => set({ text: v }, 'text')} />
		<NumberField
			label={t.textSize}
			hint={t.textSizeHint(formatMm(editor.template.font_size_pt))}
			value={body.size_pt}
			min={5}
			max={72}
			nullable
			onchange={(n) => set({ size_pt: n }, 'size_pt')}
		/>
		<Switch label={t.bold} bind:checked={() => body.bold, (v) => set({ bold: v })} />
		<div class="field">
			<span class="label">{t.align}</span>
			<SegmentedControl
				label={t.align}
				options={aligns}
				disabled={listAligned}
				hint={listAligned ? t.numberedAlignHint : undefined}
				bind:value={() => (listAligned ? 'left' : body.align), (v) => set({ align: v })}
			/>
		</div>
		<Switch
			label={t.oneLine}
			hint={t.oneLineHint}
			bind:checked={() => body.one_line, (v) => set({ one_line: v })}
		/>
		<Switch label={t.frame} bind:checked={() => body.frame, (v) => set({ frame: v })} />
		<Switch
			label={t.numbered}
			hint={body.one_line ? t.numberedOffHint : undefined}
			disabled={body.one_line}
			bind:checked={() => body.numbered, (v) => set({ numbered: v })}
		/>
	</div>
{/if}

<style>
	.props {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1-5);
	}
	.label {
		font-size: var(--text-sm);
		font-weight: 500;
	}
</style>
