<!-- The properties of the page: its size (A4, A5 or its own in millimetres), «Альбомная», the margins, the
     layout of the copies (only those the document allows) and the base size of the text. «2 на лист A4»
     needs A4 in portrait, «Два рядом» A4 in landscape: with them the size is not chosen. A sticker is printed on the label of «Печать»
     (the page of the template does not count then): it only says which one. -->
<script lang="ts">
	import type { Layout } from '$api/Layout';
	import type { Margins } from '$api/Margins';
	import type { PageSetup } from '$api/PageSetup';
	import type { Template } from '$api/Template';
	import { ru } from '#lib/i18n/ru.js';
	import { isLabel } from '#lib/print/print.svelte.js';
	import { formatMm, printSettings } from '#lib/print/settings.svelte.js';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import type { TemplateEditor } from '../editor.svelte.js';
	import NumberField from './NumberField.svelte';

	let { editor }: { editor: TemplateEditor } = $props();

	const t = ru.templates;
	const template = $derived(editor.template);
	const page = $derived(template.page);

	/** The sizes of the paper, portrait, mm. */
	const PAPER = { a4: [210, 297], a5: [148, 210] } as const;
	type SizeChoice = 'a4' | 'a5' | 'custom';
	const SIZES: { value: SizeChoice; label: string }[] = [
		{ value: 'a4', label: 'A4' },
		{ value: 'a5', label: 'A5' },
		{ value: 'custom', label: t.customSize }
	];
	const size = $derived<SizeChoice>(typeof page.size === 'string' ? page.size : 'custom');
	const custom = $derived(typeof page.size === 'string' ? null : page.size.custom);

	/** A label is printed in the size of «Печать», one copy: it has no other layout (decision 7). */
	const onLabel = $derived(isLabel(template.kind));
	const layouts = $derived<Layout[]>(
		onLabel ? ['one_per_page'] : ['two_per_a4', 'two_across', 'sheet_per_copy', 'one_per_page']
	);
	const layoutItems = $derived(layouts.map((value) => ({ value, label: t.layouts[value] })));
	const twoPerA4 = $derived(template.layout === 'two_per_a4');
	const twoAcross = $derived(template.layout === 'two_across');

	function edit(next: Template, key?: string) {
		editor.edit(next, key);
	}
	const setPage = (patch: Partial<PageSetup>, key?: string) =>
		edit({ ...template, page: { ...page, ...patch } }, key && `page.${key}`);

	function chooseSize(v: SizeChoice) {
		if (v === size) return;
		if (v !== 'custom') return setPage({ size: v });
		const [width_mm, height_mm] = PAPER[size === 'custom' ? 'a4' : size];
		setPage({ size: { custom: { width_mm, height_mm } } });
	}

	function setCustom(side: 'width_mm' | 'height_mm', n: number | null) {
		if (custom === null || n === null) return;
		setPage({ size: { custom: { ...custom, [side]: n } } }, `size.${side}`);
	}

	function setMargin(side: keyof Margins, n: number | null) {
		if (n === null) return;
		setPage({ margins_mm: { ...page.margins_mm, [side]: n } }, `margins_mm.${side}`);
	}

	function chooseLayout(layout: Layout | null) {
		if (layout === null || layout === template.layout) return;
		// Two copies on one sheet are two halves of an A4 in portrait.
		// Two copies side by side are two halves of an A4 in landscape.
		const next =
			layout === 'two_per_a4'
				? { ...page, size: 'a4' as const, landscape: false }
				: layout === 'two_across'
					? { ...page, size: 'a4' as const, landscape: true }
					: page;
		edit({ ...template, layout, page: next });
	}

	/**
	 * The least content area of a page, mm: `MIN_CONTENT_MM` of `crates/mw-domain/src/document_template.rs`
	 * (the error `no_room`). The server checks it against the page of the template; a sticker is printed on
	 * the label of this computer, which the server does not know, so the check is repeated here.
	 */
	const MIN_CONTENT_MM = 10;
	const labelNoRoom = $derived.by(() => {
		if (template.kind !== 'sticker') return null;
		const [w, h] = printSettings.labelMm;
		const { top, right, bottom, left } = page.margins_mm;
		if (w - left - right >= MIN_CONTENT_MM && h - top - bottom >= MIN_CONTENT_MM) return null;
		return t.labelNoRoom(formatMm(w), formatMm(h));
	});

	const MARGINS: (keyof Margins)[] = ['top', 'right', 'bottom', 'left'];
</script>

<div class="props">
	{#if layoutItems.length > 1}
		<Select label={t.layout} items={layoutItems} bind:value={() => template.layout, chooseLayout} />
	{/if}
	{#if onLabel}
		<p class="note">
			{t.labelPage(ru.print.presetMm(printSettings.labelMm[0], printSettings.labelMm[1]))}
		</p>
	{:else if twoPerA4}
		<p class="note">{t.twoPerA4Page}</p>
	{:else if twoAcross}
		<p class="note">{t.twoAcrossPage}</p>
	{:else}
		<div class="field">
			<span class="label">{t.size}</span>
			<SegmentedControl label={t.size} options={SIZES} bind:value={() => size, chooseSize} />
		</div>
		{#if custom}
			<div class="pair">
				<NumberField
					label={t.width}
					value={custom.width_mm}
					min={15}
					max={420}
					onchange={(n) => setCustom('width_mm', n)}
				/>
				<NumberField
					label={t.height}
					value={custom.height_mm}
					min={15}
					max={420}
					onchange={(n) => setCustom('height_mm', n)}
				/>
			</div>
		{/if}
		<Switch
			label={t.landscape}
			bind:checked={() => page.landscape, (v) => setPage({ landscape: v })}
		/>
	{/if}
	<fieldset class="margins">
		<legend class="label">{t.margins}</legend>
		<div class="pair">
			{#each MARGINS as side (side)}
				<NumberField
					label={t[side]}
					value={page.margins_mm[side]}
					min={0}
					max={50}
					onchange={(n) => setMargin(side, n)}
				/>
			{/each}
		</div>
		{#if labelNoRoom}
			<p class="warning" role="status">{labelNoRoom}</p>
		{/if}
	</fieldset>
	<NumberField
		label={t.fontSize}
		value={template.font_size_pt}
		min={5}
		max={36}
		onchange={(n) => n !== null && edit({ ...template, font_size_pt: n }, 'font_size_pt')}
	/>
</div>

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
		padding: 0;
		font-size: var(--text-sm);
		font-weight: 500;
	}
	.note {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.warning {
		margin: 0;
		color: var(--danger);
		font-size: var(--text-sm);
	}
	.pair {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-3);
	}
	.margins {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		border: 0;
		min-width: 0;
	}
</style>
