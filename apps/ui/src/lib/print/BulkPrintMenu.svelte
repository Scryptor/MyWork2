<!-- «Печать ▾» of the selection bar of the orders list (spec 7, plan 7.2): a document of every selected
     order, to the printer («Печать», only in the program) or into one PDF («Сохранить PDF»). While a batch
     is on its way the menu gives way to «Печать: 3 из 12» and «Остановить». -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Printer from '@lucide/svelte/icons/printer';
	import type { DocumentKind } from '$api/DocumentKind';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import { BATCH_DOCUMENTS, BULK_PRINT_LIMIT, documentName } from './print.svelte.js';
	import { canPrintDirectly } from './platform.js';

	let {
		count,
		progress = null,
		busy = false,
		onprint,
		onsave,
		onstop
	}: {
		/** How many orders are selected. */
		count: number;
		/** A batch is being printed: how far it is. */
		progress?: { done: number; total: number } | null;
		/** Another action of the bar runs (a PDF is being made): the menu waits. */
		busy?: boolean;
		onprint: (kind: DocumentKind) => void;
		onsave: (kind: DocumentKind) => void;
		onstop: () => void;
	} = $props();

	const t = ru.print;
	const tooMany = $derived(count > BULK_PRINT_LIMIT);

	const items = $derived.by<MenuItem[]>(() => {
		const section = (heading: string, run: (kind: DocumentKind) => void): MenuItem[] => [
			{ heading },
			...BATCH_DOCUMENTS.map((kind): MenuItem => ({
				label: documentName(kind),
				disabled: tooMany,
				onselect: () => run(kind)
			}))
		];
		return [
			...(canPrintDirectly()
				? [...section(t.batchPrintHeading, onprint), { separator: true }]
				: []),
			...section(t.batchSaveHeading, onsave)
		] as MenuItem[];
	});
</script>

{#if progress}
	<span class="progress" role="status">{t.batchProgress(progress.done, progress.total)}</span>
	<Button variant="ghost" size="sm" onclick={onstop}>{t.batchStop}</Button>
{:else}
	<DropdownMenu {items} label={t.print}>
		{#snippet trigger(props)}
			<Button
				{...props}
				variant="ghost"
				size="sm"
				icon={Printer}
				iconEnd={ChevronDown}
				disabled={busy}>{t.print}</Button
			>
		{/snippet}
	</DropdownMenu>
	{#if tooMany}<span class="hint">{t.batchLimit}</span>{/if}
{/if}

<style>
	.progress {
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.hint {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
