<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import CircleAlert from '@lucide/svelte/icons/circle-alert';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import X from '@lucide/svelte/icons/x';
	import { tick } from 'svelte';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import IconButton from '../IconButton.svelte';
	import { toast, type ToastItem } from './toast.svelte.js';

	let region = $state<HTMLElement | null>(null);
	let hovered = $state(false);
	let focused = $state(false);

	// Ctrl/⌘+Z outside fields undoes the newest undoable toast.
	$effect(() => registerHotkey('Mod+Z', () => toast.undoLast()));

	// Timers wait while the pointer or focus is on the toasts.
	$effect(() => {
		if (!hovered && !focused) return;
		toast.pause();
		return () => toast.resume();
	});

	// A region that empties under the pointer or focus gets no mouseleave or focusout.
	$effect(() => {
		if (toast.items.length > 0) return;
		hovered = false;
		focused = false;
	});

	/** The pressed button is gone with its toast: look where focus is now. */
	async function settle() {
		await tick();
		focused = region?.contains(document.activeElement) ?? false;
	}

	function act(t: ToastItem) {
		toast.dismiss(t.id);
		void t.action?.run();
		void settle();
	}

	function close(t: ToastItem) {
		toast.dismiss(t.id);
		void settle();
	}
</script>

<!-- data-mw-toaster: a press here keeps an open dialog (dialog.ts, keepOpenOnToasts). A mouse press
     leaves focus where it was, in the dialog among others; the keyboard still reaches the toasts. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
	bind:this={region}
	data-mw-toaster
	class="toaster"
	class:phone={display.phone}
	role="status"
	aria-live="polite"
	aria-atomic="false"
	aria-label={ru.toast.region}
	onmousedown={(e) => e.preventDefault()}
	onmouseenter={() => (hovered = true)}
	onmouseleave={() => (hovered = false)}
	onfocusin={() => (focused = true)}
	onfocusout={(e) => {
		if (!(e.relatedTarget instanceof Node && region?.contains(e.relatedTarget))) focused = false;
	}}
>
	{#each toast.items as t (t.id)}
		<div class="toast {t.tone ?? 'neutral'}">
			{#if t.tone === 'success'}
				<span class="icon"><CircleCheck size={16} aria-hidden="true" /></span>
			{:else if t.tone === 'danger'}
				<span class="icon"><CircleAlert size={16} aria-hidden="true" /></span>
			{/if}
			<span class="msg">{t.message}</span>
			{#if t.action}
				<button type="button" class="action" onclick={() => act(t)}>{t.action.label}</button>
			{/if}
			<IconButton label={ru.common.close} icon={X} size="sm" onclick={() => close(t)} />
		</div>
	{/each}
</div>

<style>
	/* An open modal turns pointer events off on <body>: toasts stay pressable over it. */
	.toaster {
		pointer-events: auto;
		position: fixed;
		z-index: var(--z-toast);
		left: var(--space-4);
		bottom: var(--space-4);
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
		max-width: calc(100vw - 2 * var(--space-4));
	}
	/* Phone: centred above the bottom navigation (--bottom-nav-height). */
	.toaster.phone {
		left: 50%;
		bottom: calc(var(--bottom-nav-height) + var(--space-3) + env(safe-area-inset-bottom, 0px));
		transform: translateX(-50%);
		align-items: center;
		width: calc(100vw - 2 * var(--space-4));
	}
	.toast {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		max-width: 28rem;
		min-height: var(--control-lg);
		padding: var(--space-1) var(--space-1) var(--space-1) var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow-lg);
		font-size: var(--text-md);
		animation: toast-in var(--duration-normal) var(--ease-out);
	}
	.success {
		border-color: transparent;
		background: var(--success-soft);
		color: var(--success-text);
	}
	.danger {
		border-color: transparent;
		background: var(--danger-soft);
		color: var(--danger-text);
	}
	.icon {
		display: inline-flex;
		flex: none;
	}
	.msg {
		flex: 1;
		min-width: 0;
		padding: var(--space-1) 0;
	}
	.action {
		flex: none;
		padding: var(--space-1) var(--space-2);
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--accent);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	.success .action,
	.danger .action {
		color: inherit;
		text-decoration: underline;
	}
	.action:hover {
		background: var(--surface-3);
	}
	@media (pointer: coarse) {
		.action {
			min-height: 2.75rem;
		}
	}
	@keyframes toast-in {
		from {
			opacity: 0;
			transform: translateY(var(--space-2));
		}
	}
</style>
