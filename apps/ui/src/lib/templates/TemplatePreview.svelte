<!-- The live preview of the editor (plan 7.1): the model as the server draws it, on the order chosen or the
     sample, built a moment after the last edit — one request at a time, always of the newest model. A
     sticker is drawn on the label of «Печать» with its offset and printer's resolution, an act at the
     resolution of acts: as it is printed. A model the server does not take keeps the pages drawn before,
     dimmed, under the list of its mistakes; the errors of own Typst code go to `ondiagnostics` too, for
     the lines of the code editor. The pages are `blob:` addresses, freed when they go. -->
<script lang="ts">
	import type { CodeDiagnostic } from '$api/CodeDiagnostic';
	import type { DocumentKind } from '$api/DocumentKind';
	import type { DocumentPages } from '$api/DocumentPages';
	import type { Template } from '$api/Template';
	import type { TemplatePreview } from '$api/TemplatePreview';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { previewTemplate } from '#lib/api/document-templates.js';
	import { documentQuery } from '#lib/api/documents.js';
	import { ru } from '#lib/i18n/ru.js';
	import { documentOptions } from '#lib/print/print.svelte.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import { onDestroy } from 'svelte';
	import { templateErrors } from './editor.svelte.js';
	import { createPreviewQueue } from './preview-queue.js';

	let {
		kind,
		template,
		orderId,
		ondiagnostics
	}: {
		kind: DocumentKind;
		template: Template;
		/** The order whose data is shown; null: the sample. */
		orderId: number | null;
		/**
		 * The errors of own code of the newest answer: those of a refused model, none after any other
		 * answer, so no mark stays on code it may no longer fit.
		 */
		ondiagnostics?: (diagnostics: readonly CodeDiagnostic[]) => void;
	} = $props();

	const t = ru.templates;

	/** A page of PNG in base64 as a picture of this page's own. */
	function pictureOf(base64: string): string {
		const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
		return URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
	}

	let urls = $state.raw<string[]>([]);
	/** The mistakes of the newest model (a 422), or why it was not drawn; null: drawn. */
	let failure = $state.raw<{ title?: string; lines: string[] } | null>(null);
	let loaded = $state(false);

	function show(doc: DocumentPages) {
		const made: string[] = [];
		try {
			for (const page of doc.pages) made.push(pictureOf(page));
		} catch {
			for (const url of made) URL.revokeObjectURL(url);
			failure = { lines: [ru.print.pagesFailed] };
			return;
		}
		for (const url of urls) URL.revokeObjectURL(url);
		urls = made;
		failure = null;
		loaded = true;
	}

	const queue = createPreviewQueue(
		(input: { body: TemplatePreview }, signal) => previewTemplate(kind, input.body, signal),
		{
			delayMs: 400,
			onresult: (doc) => {
				ondiagnostics?.([]);
				show(doc);
			},
			onerror: (e) => {
				ondiagnostics?.(
					e instanceof ApiError && e.code === 'validation' ? (e.body?.diagnostics ?? []) : []
				);
				loaded = true;
				failure =
					e instanceof ApiError && e.code === 'validation'
						? { title: t.previewFailed, lines: templateErrors(e, sent ?? template) }
						: { lines: [errorMessage(e)] };
			}
		}
	);
	/** The model of the newest request: the mistakes of an answer name its blocks. */
	let sent: Template | null = null;

	$effect(() => {
		// The label, its offset and the printer's resolution of this computer for a sticker; acts at theirs.
		const q = documentQuery('png', documentOptions(kind));
		const body: TemplatePreview = {
			template,
			printer_dpi: q.printer_dpi,
			label_w_mm: q.label_w_mm,
			label_h_mm: q.label_h_mm,
			offset_x_mm: q.offset_x_mm,
			offset_y_mm: q.offset_y_mm
		};
		if (orderId !== null) body.order_id = orderId;
		sent = template;
		queue.request({ body });
	});

	onDestroy(() => {
		queue.dispose();
		for (const url of urls) URL.revokeObjectURL(url);
	});
</script>

<div class="preview" aria-busy={!loaded}>
	{#if failure}
		<Callout tone="danger" title={failure.title}>
			{#if failure.lines.length === 1}
				{failure.lines[0]}
			{:else}
				<ul class="errors">
					{#each failure.lines as line, i (i)}<li>{line}</li>{/each}
				</ul>
			{/if}
		</Callout>
	{/if}
	{#if urls.length > 0}
		<div class="pages" class:stale={failure !== null}>
			{#each urls as url, i (url)}
				<figure class="page">
					<img src={url} alt={ru.print.page(i + 1, urls.length)} />
					<figcaption aria-hidden="true">{ru.print.page(i + 1, urls.length)}</figcaption>
				</figure>
			{/each}
		</div>
	{:else if !loaded}
		<div aria-label={t.previewLoading}>
			<Skeleton height="24rem" />
		</div>
	{/if}
</div>

<style>
	.preview {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.errors {
		margin: 0;
		padding-inline-start: var(--space-4);
	}
	.pages {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		transition: opacity 0.15s;
	}
	.pages.stale {
		opacity: 0.45;
	}
	.page {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
	}
	.page img {
		display: block;
		width: 100%;
		height: auto;
		border: 1px solid var(--border);
		background: var(--surface);
		box-shadow: var(--shadow-md);
	}
	figcaption {
		color: var(--text-3);
		font-size: var(--text-xs);
		text-align: center;
	}
</style>
