<!-- The template editor (plan 7.1, spec 5.1 and section 7 of the interface): on top the document and the
     order for the example, «Отменить», «Повторить», «Сбросить к стандартному» and the save state; below
     three columns — the blocks, the live preview, the properties of the chosen item (as wide as the preview
     while a «Typst-код» block is chosen). Medium screens put the properties under the blocks; narrow ones
     show one part at a time. Ctrl/⌘Z outside the fields undoes a step of the template (newer than the
     toasts' own Ctrl/⌘Z, so it wins while the editor is open). -->
<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import Redo2 from '@lucide/svelte/icons/redo-2';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { DocumentKind } from '$api/DocumentKind';
	import { listTemplates } from '#lib/api/document-templates.js';
	import { listOrders } from '#lib/api/orders.js';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { display } from '#lib/design/prefs.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import SaveIndicator from '#lib/orders/SaveIndicator.svelte';
	import { deviceOf } from '#lib/orders/view.js';
	import { documentName } from '#lib/print/print.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';
	import BlockList from './BlockList.svelte';
	import type { TemplateEditor } from './editor.svelte.js';
	import { createOrderSearch } from './order-search.svelte.js';
	import { blockAt } from './ops.js';
	import Properties from './Properties.svelte';
	import TemplateFileMenu from './TemplateFileMenu.svelte';
	import TemplatePreview from './TemplatePreview.svelte';

	let { editor }: { editor: TemplateEditor } = $props();

	const t = ru.templates;
	const kind = $derived(editor.kind);

	// «Документ»: every document with a template; the one shown until the list comes.
	let kinds = $state.raw<DocumentKind[]>([]);
	$effect(() => {
		let current = true;
		listTemplates().then(
			(list) => {
				if (current) kinds = list.map((i) => i.kind).filter((k) => k !== 'calibration');
			},
			() => {
				// Without the list the document shown is the only one to choose.
			}
		);
		return () => {
			current = false;
		};
	});
	const kindItems = $derived(
		(kinds.includes(kind) ? kinds : [kind]).map((k) => ({ value: k, label: documentName(k) }))
	);
	function chooseKind(k: DocumentKind | null) {
		if (k !== null && k !== kind) void goto(resolve(`templates/${k}`));
	}

	// «Заказ для примера»: «Пример» (the sample order), or an order found by its number, client or device.
	/** The blank act has no order: its preview is the blank form. */
	const withOrder = $derived(kind !== 'blank_intake_act');
	const orders = createOrderSearch({
		sample: t.sample,
		sampleHint: t.sampleHint,
		find: async (query) =>
			(await listOrders({ query, limit: 20 })).items.map((o) => ({
				id: o.id,
				label: `${ru.order.title(o.id)} · ${deviceOf(o)}`,
				hint: o.client_name || undefined
			}))
	});
	$effect(() => () => orders.dispose());

	// The template's undo and redo, outside the fields: in a field the browser undoes what is typed.
	$effect(() => {
		const offs = [
			registerHotkey('Mod+Z', () => editor.undo()),
			registerHotkey('Mod+Shift+Z', () => editor.redo()),
			registerHotkey('Mod+Y', () => editor.redo())
		];
		return () => offs.forEach((off) => off());
	});

	/** The texts of a file the document does not take; null: nothing to show. */
	let misfit = $state.raw<string[] | null>(null);

	/** A code block is chosen: on a wide screen its code takes half of what the blocks leave. */
	const coding = $derived.by(() => {
		const s = editor.selected;
		return s !== 'page' && blockAt(editor.template, s)?.body.type === 'typst_code';
	});

	type Pane = 'blocks' | 'properties' | 'preview';
	let pane = $state<Pane>('blocks');
	const PANES: { value: Pane; label: string }[] = [
		{ value: 'blocks', label: t.blocks },
		{ value: 'properties', label: t.properties },
		{ value: 'preview', label: t.preview }
	];
</script>

<div class="editor" class:coding>
	<header class="head">
		<div class="title">
			<Button variant="ghost" size="sm" icon={ArrowLeft} href={resolve('settings/documents')}
				>{ru.settings.documents}</Button
			>
			<h1>{t.title}</h1>
			<p class="meta">
				<span>{editor.isStandard ? t.standard : t.custom}</span>
				<SaveIndicator editor={editor.status} />
			</p>
		</div>
		<div class="tools">
			<div class="pick">
				<Select label={ru.print.document} items={kindItems} bind:value={() => kind, chooseKind} />
			</div>
			{#if withOrder}
				<div class="pick">
					<Combobox
						label={t.sampleOrder}
						items={orders.items}
						filter={false}
						bind:value={() => orders.text, orders.setText}
						onselect={orders.pick}
						onblur={orders.leave}
					/>
				</div>
			{/if}
			<div class="actions">
				<IconButton
					label={t.undo}
					icon={Undo2}
					variant="secondary"
					disabled={!editor.canUndo}
					onclick={() => editor.undo()}
				/>
				<IconButton
					label={t.redo}
					icon={Redo2}
					variant="secondary"
					disabled={!editor.canRedo}
					onclick={() => editor.redo()}
				/>
				<TemplateFileMenu {editor} onmisfit={(m) => (misfit = m)} />
				<Button
					icon={RotateCcw}
					disabled={editor.isStandard}
					onclick={() => editor.resetToStandard()}>{t.reset}</Button
				>
			</div>
		</div>
		{#if misfit}
			<Callout tone="warning" title={t.misfitFile}>
				<ul class="misfit">
					{#each misfit as message, i (i)}<li>{message}</li>{/each}
				</ul>
				{#snippet actions()}
					<Button size="sm" onclick={() => (misfit = null)}>{ru.common.close}</Button>
				{/snippet}
			</Callout>
		{/if}
		{#if editor.conflict}
			<Callout tone="warning" title={t.conflictTitle}>
				{t.conflictText}
				{#snippet actions()}
					<Button size="sm" onclick={() => editor.keepMine()}>{ru.order.keepMine}</Button>
					<Button size="sm" onclick={() => editor.takeTheirs()}>{ru.order.takeNew}</Button>
				{/snippet}
			</Callout>
		{/if}
		{#if display.narrow}
			<SegmentedControl label={t.panes} options={PANES} bind:value={pane} />
		{/if}
	</header>

	<div class="body">
		<section class="pane blocks" class:shown={pane === 'blocks'} aria-label={t.blocks}>
			<BlockList {editor} />
		</section>
		<section class="pane preview" class:shown={pane === 'preview'} aria-label={t.preview}>
			<TemplatePreview
				{kind}
				template={editor.template}
				orderId={withOrder ? orders.orderId : null}
				ondiagnostics={(d) => (editor.diagnostics = d)}
			/>
		</section>
		<section class="pane props" class:shown={pane === 'properties'} aria-label={t.properties}>
			<Properties {editor} />
		</section>
	</div>
</div>

<style>
	.editor {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		box-sizing: border-box;
		height: 100%;
		padding: var(--space-3) var(--space-4) 0;
	}
	.head {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.title {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
	}
	h1 {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1-5);
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.tools {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--space-3);
	}
	.misfit {
		margin: 0;
		padding-inline-start: var(--space-4);
	}
	.pick {
		width: 16rem;
		max-width: 100%;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	/* Wide: the blocks, the preview, the properties side by side, each scrolled on its own. */
	.body {
		display: grid;
		flex: 1;
		grid-template-columns: 16.25rem minmax(0, 1fr) 20rem;
		grid-template-areas: 'blocks preview props';
		gap: var(--space-4);
		min-height: 0;
	}
	.pane {
		min-width: 0;
		min-height: 0;
		overflow-y: auto;
		padding-bottom: var(--space-4);
	}
	.blocks {
		grid-area: blocks;
	}
	.preview {
		grid-area: preview;
		padding-inline: var(--space-1);
	}
	.props {
		grid-area: props;
	}
	/* Wide with a code block chosen: the code as wide as the preview. */
	:global([data-width='lg']) .coding .body,
	:global([data-width='xl']) .coding .body {
		grid-template-columns: 16.25rem minmax(0, 1fr) minmax(0, 1fr);
	}
	/* Medium: the properties under the blocks, the preview beside them. */
	:global([data-width='md']) .body {
		grid-template-columns: 18.75rem minmax(0, 1fr);
		grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
		grid-template-areas:
			'blocks preview'
			'props preview';
	}
	/* Narrow: one part at a time, chosen above; the page scrolls as a whole. */
	:global([data-width='sm']) .editor,
	:global([data-width='xs']) .editor {
		height: auto;
	}
	:global([data-width='sm']) .body,
	:global([data-width='xs']) .body {
		display: block;
	}
	:global([data-width='sm']) .pane:not(.shown),
	:global([data-width='xs']) .pane:not(.shown) {
		display: none;
	}
	:global([data-width='sm']) .pane,
	:global([data-width='xs']) .pane {
		overflow: visible;
	}
	:global([data-width='xs']) .editor {
		padding-inline: var(--space-3);
	}
	:global([data-width='xs']) .pick {
		width: 100%;
	}
</style>
