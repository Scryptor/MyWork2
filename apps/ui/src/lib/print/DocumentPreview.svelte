<!-- The preview of a document (spec 7, plan 6): its pages as pictures, «Сохранить PDF» under the server's
     file name and, in the desktop program, «Печать» (Ctrl/⌘P too; in a browser the key only keeps the
     browser's printing of the page away). A document of several pages has a check box under each page
     (plan 8.2): both buttons take only the checked pages, and with none checked neither works. The
     pictures are `blob:` addresses: they are freed when the preview closes or shows another document, and
     a request still on its way then is called off; what was shown goes with them, the check boxes too.
     Without `request` it is closed. -->
<script lang="ts">
	import Download from '@lucide/svelte/icons/download';
	import Printer from '@lucide/svelte/icons/printer';
	import { untrack } from 'svelte';
	import type { DocumentKind } from '$api/DocumentKind';
	import type { DocumentPages } from '$api/DocumentPages';
	import { errorMessage } from '#lib/api/client.js';
	import type { DocumentTarget } from '#lib/api/documents.js';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Checkbox from '#lib/ui/Checkbox.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Sheet from '#lib/ui/Sheet.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import { pageSelection } from './pages.js';
	import { canPrintDirectly } from './platform.js';
	import {
		documentName,
		documentTitle,
		previewPages,
		printDocument,
		savePdf,
		type PreviewRequest,
		type PrintOptions
	} from './print.svelte.js';

	type Target = DocumentTarget;

	let {
		request,
		load = previewPages,
		save = savePdf,
		print = canPrintDirectly() ? printDocument : null,
		onclose
	}: {
		/** What is shown; null: the preview is closed. */
		request: PreviewRequest | null;
		/** The pages of a document (the server's by default; the showcase has its own). */
		load?: (kind: DocumentKind, target: Target, signal: AbortSignal) => Promise<DocumentPages>;
		/** «Сохранить PDF» of the checked pages (`undefined` — all of them); throws what to say. */
		save?: (kind: DocumentKind, target: Target, pages?: string) => Promise<void>;
		/** «Печать»: only where the program prints by itself; null — no button. */
		print?: ((kind: DocumentKind, target: Target, opts: PrintOptions) => void) | null;
		/** The preview is closed (✕, Esc, «Печать»). */
		onclose: () => void;
	} = $props();

	const t = ru.print;

	/** The document shown: the one asked for, or another of the same order chosen here. */
	let kind = $state<DocumentKind>('intake_act');
	$effect.pre(() => {
		const r = request;
		if (r) untrack(() => (kind = r.kind));
	});
	const kinds = $derived(request?.kinds ?? []);
	const kindItems = $derived(kinds.map((k) => ({ value: k, label: documentName(k) })));

	interface Shown {
		urls: string[];
		widthMm: number;
	}
	let shown = $state.raw<Shown | null>(null);
	/** The check boxes of the pages shown, a page each; every page is checked when it comes. */
	let checked = $state<boolean[]>([]);
	/** The checked pages for the server: null — all of them, '' — none (nothing to print). */
	const selection = $derived(pageSelection(checked));
	const nothingChecked = $derived(selection === '');
	let error = $state<string | null>(null);
	/** «Повторить» asks again. */
	let attempt = $state(0);

	/** A page of PNG in base64 as a picture of this page's own. */
	function pictureOf(base64: string): string {
		const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
		return URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
	}

	$effect(() => {
		const r = request;
		const k = kind;
		void attempt;
		if (!r) return;
		const controller = new AbortController();
		const urls: string[] = [];
		const free = () => {
			for (const url of urls.splice(0)) URL.revokeObjectURL(url);
		};
		load(k, r.target, controller.signal).then(
			(doc) => {
				if (controller.signal.aborted) return;
				try {
					// Each address is kept as it is made: a broken page frees the ones before it.
					for (const page of doc.pages) urls.push(pictureOf(page));
				} catch {
					free();
					error = t.pagesFailed;
					return;
				}
				checked = urls.map(() => true);
				shown = { urls, widthMm: doc.width_mm };
			},
			(e: unknown) => {
				if (!controller.signal.aborted) error = errorMessage(e);
			}
		);
		return () => {
			controller.abort();
			free();
			// Opened again or on another document, it starts from nothing: no freed page, no old failure.
			shown = null;
			checked = [];
			error = null;
		};
	});

	// Ctrl/⌘P over the preview is its «Печать», in fields too; in a browser it only takes the key.
	$effect(() => {
		if (!request) return;
		return registerHotkey(
			'Mod+P',
			(e) => {
				if (!e.repeat) printShown();
			},
			{ allowInFields: true, allowWithOverlay: true }
		);
	});

	let saving = $state(false);
	let saveError = $state<string | null>(null);
	$effect.pre(() => {
		void request;
		void kind;
		untrack(() => (saveError = null));
	});

	async function saveShown() {
		const r = request;
		if (!r || saving || nothingChecked) return;
		saving = true;
		saveError = null;
		try {
			await save(kind, r.target, selection ?? undefined);
		} catch (e) {
			saveError = errorMessage(e);
		} finally {
			saving = false;
		}
	}

	function printShown() {
		const r = request;
		if (!r || !print || nothingChecked) return;
		const k = kind;
		const pages = selection;
		onclose();
		print(k, r.target, pages === null ? { noPreview: true } : { noPreview: true, pages });
	}

	const title = $derived(request ? documentTitle(kind, request.target) : '');
</script>

<Sheet bind:open={() => request !== null, (v) => !v && onclose()} {title}>
	{#snippet header()}
		{#if kindItems.length > 1}
			<div class="kind">
				<Select
					label={t.document}
					items={kindItems}
					bind:value={() => kind, (v) => v && (kind = v)}
				/>
			</div>
		{/if}
	{/snippet}

	{#if error}
		<Callout tone="danger">
			{error}
			{#snippet actions()}
				<Button size="sm" onclick={() => attempt++}>{ru.common.retry}</Button>
			{/snippet}
		</Callout>
	{:else if shown}
		<div class="pages" style:--page-width="{shown.widthMm}mm">
			{#each shown.urls as url, i (url)}
				<div class="sheet">
					<img class="page" src={url} alt={t.page(i + 1, shown.urls.length)} />
					{#if shown.urls.length > 1}
						<Checkbox
							label={t.pageToPrint(i + 1)}
							bind:checked={() => checked[i] ?? false, (v) => (checked[i] = v)}
						/>
					{/if}
				</div>
			{/each}
		</div>
	{:else}
		<div class="loading" aria-busy="true" aria-label={t.pagesLoading}>
			<Skeleton height="24rem" />
		</div>
	{/if}

	{#snippet footer()}
		{#if saveError}<p class="failed" role="alert">{saveError}</p>{/if}
		{#if nothingChecked}<p class="none">{t.noPagesChecked}</p>{/if}
		<Button icon={Download} busy={saving} disabled={nothingChecked} onclick={() => void saveShown()}
			>{t.savePdf}</Button
		>
		{#if print}
			<Button variant="primary" icon={Printer} disabled={nothingChecked} onclick={printShown}
				>{t.printButton}</Button
			>
		{/if}
	{/snippet}
</Sheet>

<style>
	.kind {
		max-width: 20rem;
	}
	.pages {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-4);
		padding: var(--space-2) 0 var(--space-4);
	}
	/* A page and, under it, its check box when there are several. */
	.sheet {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		max-width: 100%;
	}
	/* A page in its own size where it fits (a label is small), never wider than the sheet. */
	.page {
		display: block;
		width: var(--page-width);
		max-width: 100%;
		height: auto;
		border: 1px solid var(--border);
		box-shadow: var(--shadow-md);
	}
	.loading {
		padding: var(--space-2) 0;
	}
	.failed {
		flex: 1 0 100%;
		margin: 0;
		color: var(--danger-text);
		text-align: end;
	}
	.none {
		flex: 1 0 100%;
		margin: 0;
		color: var(--text-3);
		text-align: end;
	}
</style>
