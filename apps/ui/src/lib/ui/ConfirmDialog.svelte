<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import { AlertDialog } from 'bits-ui';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import X from '@lucide/svelte/icons/x';
	import { untrack } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { dialogFocus } from './dialog.js';
	import Button from './Button.svelte';
	import './dialog.css';

	let {
		open = $bindable(false),
		title,
		description,
		confirmLabel,
		cancelLabel = ru.common.cancel,
		tone = 'danger',
		onconfirm
	}: {
		open: boolean;
		title: string;
		description: string;
		confirmLabel: string;
		/** «Отмена» by default. */
		cancelLabel?: string;
		tone?: 'danger' | 'primary';
		onconfirm: () => void | Promise<void>;
	} = $props();

	// Phone: the layer fills the screen and closes with «Назад» (←) at the top left.
	const CloseIcon = $derived(display.phone ? ArrowLeft : X);
	let busy = $state(false);
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

	/** Stays open and busy until onconfirm settles; a failed onconfirm keeps the dialog open. */
	async function confirm() {
		busy = true;
		try {
			await onconfirm();
			open = false;
		} finally {
			busy = false;
		}
	}

	function cancel() {
		if (!busy) open = false;
	}
</script>

<AlertDialog.Root bind:open>
	<AlertDialog.Portal>
		<AlertDialog.Overlay class="mw-scrim" />
		<AlertDialog.Content
			bind:ref={content}
			onCloseAutoFocus={focusReturn.closed}
			class="mw-dialog sm {display.phone ? 'phone' : ''}"
			escapeKeydownBehavior={busy ? 'ignore' : 'close'}
		>
			<div class="mw-dialog-head">
				<AlertDialog.Title class="mw-dialog-title" level={2}>{title}</AlertDialog.Title>
			</div>
			<div class="mw-dialog-body">
				<AlertDialog.Description class="mw-dialog-desc">{description}</AlertDialog.Description>
			</div>
			<div class="mw-dialog-foot">
				<Button variant="secondary" disabled={busy} onclick={cancel}>{cancelLabel}</Button>
				<Button variant={tone} {busy} onclick={confirm}>{confirmLabel}</Button>
			</div>
			<!-- No tooltip here: an open bits-ui tooltip pauses the dialog's focus trap. -->
			<button
				type="button"
				class="mw-dialog-close"
				aria-label={display.phone ? ru.common.back : ru.common.close}
				disabled={busy}
				onclick={cancel}
			>
				<CloseIcon size={16} aria-hidden="true" />
			</button>
		</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>
