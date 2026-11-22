<script lang="ts">
	import type { DocumentKind } from '$api/DocumentKind';
	import type { DocumentPages } from '$api/DocumentPages';
	import { demoPage } from '#lib/demo/pages.js';
	import DocumentPreview from '#lib/print/DocumentPreview.svelte';
	import PrintMenu from '#lib/print/PrintMenu.svelte';
	import { documentName, isLabel, type PreviewRequest } from '#lib/print/print.svelte.js';

	const ORDER = 10481;

	let picked = $state('');
	let request = $state.raw<PreviewRequest | null>(null);

	function preview(kind: DocumentKind, kinds: readonly DocumentKind[]) {
		request = { kind, target: { orderId: ORDER }, kinds };
	}

	// Pages without a server: drawn here a moment later, an act on A4 or a label of 58 × 40 mm.
	async function load(kind: DocumentKind): Promise<DocumentPages> {
		await new Promise((resolve) => setTimeout(resolve, 400));
		const [width, height] = isLabel(kind) ? [58, 40] : [210, 297];
		return { dpi: 110, width_mm: width, height_mm: height, pages: [demoPage(width, height)] };
	}
</script>

<p class="muted">
	«Печать ▾» заказа: пункт сразу печатает (в программе), «Предпросмотр…» открывает страницы. У
	выданного заказа — ещё гарантийные акты; на телефоне кнопка — значок.
</p>
<div class="row">
	<PrintMenu
		issued={false}
		wasIssued={false}
		onprint={(kind) => (picked = documentName(kind))}
		onpreview={preview}
	/>
	<PrintMenu
		issued
		wasIssued
		compact
		onprint={(kind) => (picked = documentName(kind))}
		onpreview={preview}
	/>
	<span class="muted">Напечатано: {picked || '—'}</span>
</div>

<DocumentPreview
	{request}
	{load}
	save={async () => {
		picked = 'PDF сохранён';
	}}
	print={(kind) => (picked = documentName(kind))}
	onclose={() => (request = null)}
/>

<style>
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}
	.muted {
		margin: 0;
		color: var(--text-3);
	}
</style>
