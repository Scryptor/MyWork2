<script lang="ts">
	import { Dialog } from 'bits-ui';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import X from '@lucide/svelte/icons/x';
	import { untrack, type Snippet } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import './dialog.css';

	let {
		open = $bindable(false),
		title,
		width = 'min(70rem, 100vw - 4rem)',
		header,
		children,
		footer,
		onclose
	}: {
		open: boolean;
		title: string;
		width?: string;
		header?: Snippet;
		children: Snippet;
		footer?: Snippet;
		onclose?: () => void;
	} = $props();

	const phone = new MediaQuery(PHONE_QUERY);
	// Phone: the layer fills the screen and closes with «Назад» (←) at the top left.
	const CloseIcon = $derived(phone.current ? ArrowLeft : X);

	$effect(() => {
		if (open) return untrack(() => layers.open('modal', () => (open = false)));
	});

	function close() {
		open = false;
		onclose?.();
	}
</script>

<Dialog.Root
	bind:open
	onOpenChange={(v) => {
		if (!v) onclose?.();
	}}
>
	<Dialog.Portal>
		<Dialog.Overlay class="mw-scrim" />
		<Dialog.Content
			class="mw-dialog mw-sheet {phone.current ? 'phone' : ''}"
			style="--sheet-width: {width}"
		>
			<div class="mw-dialog-head">
				<Dialog.Title class="mw-dialog-title" level={2}>{title}</Dialog.Title>
				{@render header?.()}
			</div>
			<div class="mw-dialog-body">{@render children()}</div>
			{#if footer}<div class="mw-dialog-foot">{@render footer()}</div>{/if}
			<!-- No tooltip here: an open bits-ui tooltip pauses the dialog's focus trap. -->
			<button
				type="button"
				class="mw-dialog-close"
				aria-label={phone.current ? ru.common.back : ru.common.close}
				onclick={close}
			>
				<CloseIcon size={16} aria-hidden="true" />
			</button>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
