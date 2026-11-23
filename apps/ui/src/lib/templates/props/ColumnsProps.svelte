<!-- The properties of the columns: how many (2–4) and the gap between them. Fewer columns drop the last ones with their blocks, as one edit that can be undone. -->
<script lang="ts">
	import type { BlockBody } from '$api/BlockBody';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import NumberField from './NumberField.svelte';
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';

	type Body = Extract<BlockBody, { type: 'columns' }>;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'columns' ? block.body : null);
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

	const COUNTS = [2, 3, 4];
	const counts = COUNTS.map((n) => ({ value: n, label: String(n) }));

	function resize(n: number) {
		if (!body || n === body.columns.length) return;
		const columns = body.columns.slice(0, n);
		while (columns.length < n) columns.push([]);
		set({ columns });
	}
</script>

{#if body}
	<div class="props">
		<div class="field">
			<span class="label">{t.columnCount}</span>
			<SegmentedControl
				label={t.columnCount}
				options={counts}
				bind:value={() => body.columns.length, resize}
			/>
			<p class="hint">{t.columnsShrinkHint}</p>
		</div>
		<NumberField
			label={t.columnGap}
			value={body.gap_mm}
			min={0}
			max={30}
			onchange={(n) => n !== null && set({ gap_mm: n }, 'gap_mm')}
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
	.hint {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-xs);
	}
</style>
