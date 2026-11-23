<!-- The properties of a barcode: the height of the bars, the digits under them, the alignment, and the two ways to grow it. A column holds no barcode that fills the height. -->
<script lang="ts">
	import type { Align } from '$api/Align';
	import type { BlockBody } from '$api/BlockBody';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import NumberField from './NumberField.svelte';
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';

	type Body = Extract<BlockBody, { type: 'barcode' }>;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'barcode' ? block.body : null);
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
	/** A barcode of a column: `fill` is not allowed there. */
	const inColumn = $derived(path.length === 3);
</script>

{#if body}
	<div class="props">
		<NumberField
			label={t.barcodeHeight}
			value={body.height_mm}
			min={4}
			max={60}
			onchange={(n) => n !== null && set({ height_mm: n }, 'height_mm')}
		/>
		<Switch label={t.barcodeDigits} bind:checked={() => body.digits, (v) => set({ digits: v })} />
		<div class="field">
			<span class="label">{t.align}</span>
			<SegmentedControl
				label={t.align}
				options={aligns}
				bind:value={() => body.align, (v) => set({ align: v })}
			/>
		</div>
		<Switch
			label={t.barcodeStretch}
			bind:checked={() => body.stretch, (v) => set({ stretch: v })}
		/>
		<Switch
			label={t.barcodeFill}
			hint={inColumn ? t.barcodeFillInColumn : t.barcodeFillHint}
			disabled={inColumn}
			bind:checked={() => body.fill, (v) => set({ fill: v })}
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
