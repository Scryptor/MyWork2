<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import { Dialog } from 'bits-ui';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import X from '@lucide/svelte/icons/x';
	import { untrack, type Snippet } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { dialogFocus, keepOpenOnToasts } from './dialog.js';
	import './dialog.css';

	let {
		open = $bindable(false),
		title,
		description,
		size = 'md',
		position = 'center',
		children,
		footer,
		dismissible = true,
		onclose
	}: {
		open: boolean;
		title: string;
		description?: string;
		size?: 'sm' | 'md' | 'lg';
		/** `bottom`: on a phone the dialog rises from the bottom edge (a short list of choices)
		 *  instead of filling the screen. */
		position?: 'center' | 'bottom';
		children: Snippet;
		footer?: Snippet;
		/** `false` while a request runs: Esc, a click outside and «✕» leave the dialog open. */
		dismissible?: boolean;
		onclose?: () => void;
	} = $props();

	// Phone: the layer fills the screen and closes with «Назад» (←) at the top left.
	const CloseIcon = $derived(display.phone ? ArrowLeft : X);

	let content = $state<HTMLElement | null>(null);
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
			bind:ref={content}
			escapeKeydownBehavior={dismissible ? 'close' : 'ignore'}
			onInteractOutside={(e) => {
				keepOpenOnToasts(e);
				if (!dismissible) e.preventDefault();
			}}
			onCloseAutoFocus={focusReturn.closed}
			class="mw-dialog {size} {position === 'bottom' ? 'bottom' : ''} {display.phone
				? 'phone'
				: ''}"
		>
			<div class="mw-dialog-head">
				<Dialog.Title class="mw-dialog-title" level={2}>{title}</Dialog.Title>
				{#if description}
					<Dialog.Description class="mw-dialog-desc">{description}</Dialog.Description>
				{/if}
			</div>
			<div class="mw-dialog-body">{@render children()}</div>
			{#if footer}<div class="mw-dialog-foot">{@render footer()}</div>{/if}
			<!-- No tooltip here: an open bits-ui tooltip pauses the dialog's focus trap. -->
			<button
				type="button"
				class="mw-dialog-close"
				aria-label={display.phone ? ru.common.back : ru.common.close}
				disabled={!dismissible}
				onclick={close}
			>
				<CloseIcon size={16} aria-hidden="true" />
			</button>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
