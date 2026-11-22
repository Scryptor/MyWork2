<!-- «Печать ▾» of an order (spec 7, plan 6): the acts and the sticker, the warranty acts of an order that
     was issued, and «Предпросмотр…» of the act Ctrl/⌘P prints, where the others can be chosen too. On a
     phone the button is an icon. -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Printer from '@lucide/svelte/icons/printer';
	import type { DocumentKind } from '$api/DocumentKind';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { documentName, menuDocuments, shortcutKind } from './print.svelte.js';

	let {
		issued,
		wasIssued,
		compact = false,
		variant = 'secondary',
		onprint,
		onpreview
	}: {
		/** The order is issued: Ctrl/⌘P prints its issue act, not the intake act. */
		issued: boolean;
		/** It has a date of issue: the warranty acts are there too. */
		wasIssued: boolean;
		/** Only the icon (a phone). */
		compact?: boolean;
		variant?: 'ghost' | 'secondary';
		onprint: (kind: DocumentKind) => void;
		/** «Предпросмотр…»: the act of Ctrl/⌘P, and the documents of the menu to switch to. */
		onpreview: (kind: DocumentKind, kinds: readonly DocumentKind[]) => void;
	} = $props();

	const t = ru.print;

	const items = $derived.by<MenuItem[]>(() => {
		const kinds = menuDocuments(wasIssued);
		const main = shortcutKind(issued);
		const item = (kind: DocumentKind): MenuItem => ({
			label: documentName(kind),
			shortcut: kind === main ? 'Mod+P' : undefined,
			onselect: () => onprint(kind)
		});
		const warranty = kinds.filter((k) => k.startsWith('warranty_'));
		return [
			...kinds.filter((k) => !warranty.includes(k)).map(item),
			{ separator: true },
			...warranty.map(item),
			{ label: t.previewItem, onselect: () => onpreview(main, kinds) }
		];
	});
</script>

<DropdownMenu {items} label={t.print}>
	{#snippet trigger(props)}
		{#if compact}
			<IconButton {...props} label={t.print} icon={Printer} {variant} />
		{:else}
			<Button {...props} {variant} icon={Printer} iconEnd={ChevronDown}>{t.print}</Button>
		{/if}
	{/snippet}
</DropdownMenu>
