<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import { Dialog } from 'bits-ui';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import X from '@lucide/svelte/icons/x';
	import { tick, untrack, type Snippet } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { navigating } from '$app/state';
	import { isEditableTarget } from '#lib/design/editable.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { dialogFocus, keepOpenOnToasts } from './dialog.js';
	import './dialog.css';

	let {
		open = $bindable(false),
		title,
		width = 'min(64rem, 100vw - 4rem)',
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

	// Phone: the layer fills the screen and closes with «Назад» (←) at the top left.
	const CloseIcon = $derived(display.phone ? ArrowLeft : X);

	const focusReturn = dialogFocus();
	$effect(() => {
		if (!open) return;
		return untrack(() => {
			focusReturn.opened();
			return layers.open(
				'modal',
				() => (open = false),
				() => content
			);
		});
	});

	function close() {
		open = false;
		onclose?.();
	}

	let content = $state<HTMLElement | null>(null);
	let navigated: (() => void)[] = [];
	afterNavigate(() => {
		for (const done of navigated.splice(0)) done();
	});

	/**
	 * The first field takes focus once SvelteKit is done with its own focus after a navigation (a sheet
	 * opened by a key that navigates, or by the page it navigates to). Earlier, SvelteKit takes focus to
	 * <body>, and WebKit loses the caret: typing goes nowhere.
	 */
	async function focusFirstField(e: Event) {
		e.preventDefault();
		if (navigating.to !== null) {
			// Meanwhile the sheet itself holds focus: SvelteKit's focus to <body> comes back here, not to a field.
			content?.focus({ preventScroll: true });
			await new Promise<void>((done) => navigated.push(done));
		}
		await tick();
		if (!open || !content) return;
		const field = [
			...content.querySelectorAll<HTMLElement>('input, textarea, select, [contenteditable]')
		].find(
			(el) =>
				isEditableTarget(el) &&
				el.tabIndex >= 0 &&
				!el.matches(':disabled') &&
				el.getClientRects().length > 0
		);
		(field ?? content).focus({ preventScroll: true });
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
			onInteractOutside={keepOpenOnToasts}
			onCloseAutoFocus={focusReturn.closed}
			onOpenAutoFocus={focusFirstField}
			bind:ref={content}
			class="mw-dialog mw-sheet {display.phone ? 'phone' : ''}"
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
				aria-label={display.phone ? ru.common.back : ru.common.close}
				onclick={close}
			>
				<CloseIcon size={16} aria-hidden="true" />
			</button>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
