<script lang="ts">
	import { Portal } from 'bits-ui';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import X from '@lucide/svelte/icons/x';
	import { tick, untrack, type Snippet } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { isEditableTarget, registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import IconButton from './IconButton.svelte';

	let {
		open = $bindable(false),
		title,
		width = 'min(30rem, 100vw - 3rem)',
		actions,
		header,
		children,
		footer,
		onclose,
		onprev,
		onnext,
		returnFocusTo = null
	}: {
		open: boolean;
		title: string;
		width?: string;
		/** Buttons in the top bar after the title, e.g. «⋯» on a phone (spec 6.12). */
		actions?: Snippet;
		header?: Snippet;
		children: Snippet;
		footer?: Snippet;
		onclose?: () => void;
		/** May return a promise: the move waits for it (the panel saves first). */
		onprev?: () => unknown;
		onnext?: () => unknown;
		returnFocusTo?: HTMLElement | null;
	} = $props();

	const titleId = $props.id();
	const phone = new MediaQuery(PHONE_QUERY);
	let panel: HTMLElement | null = null;

	function close() {
		open = false;
		onclose?.();
	}

	/** While the panel is in the DOM: a 'panel' layer (Esc closes it), focus inside, focus back. */
	function lifecycle(node: HTMLElement) {
		return untrack(() => {
			const previous = document.activeElement;
			const off = layers.open('panel', close);
			panel = node;
			node.focus({ preventScroll: true });
			return () => {
				off();
				panel = null;
				// The page stays live under the panel: focus the user moved there stays there.
				const active = document.activeElement;
				if (active && active !== document.body && !node.contains(active)) return;
				const target = returnFocusTo ?? previous;
				if (target instanceof HTMLElement && target.isConnected) {
					target.focus({ preventScroll: true });
				}
			};
		});
	}

	// Alt+↑/↓ go to the neighbouring record from anywhere, fields included, while the panel is open.
	$effect(() => {
		if (!open) return;
		const prev = onprev;
		const next = onnext;
		const offs = [
			prev ? registerHotkey('Alt+ArrowUp', () => void step(prev), { allowInFields: true }) : null,
			next ? registerHotkey('Alt+ArrowDown', () => void step(next), { allowInFields: true }) : null
		];
		return () => {
			for (const off of offs) off?.();
		};
	});

	/** Native controls that move their own value with the arrows. */
	function ownsArrows(target: EventTarget | null): boolean {
		return (
			target instanceof HTMLInputElement && (target.type === 'radio' || target.type === 'range')
		);
	}

	// ↑/↓ with focus in the panel; fields and widgets that handled the arrow (bits-ui) keep it.
	function onkeydown(e: KeyboardEvent) {
		if (e.defaultPrevented || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
		const go = e.key === 'ArrowUp' ? onprev : e.key === 'ArrowDown' ? onnext : undefined;
		if (!go || isEditableTarget(e.target) || ownsArrows(e.target)) return;
		e.preventDefault();
		void step(go);
	}

	/**
	 * Every move to the neighbouring record (‹ ›, ↑↓, Alt+↑↓). On the first or last record the focused
	 * ‹ or › turns disabled and the browser drops focus to the page: keep it in the panel.
	 */
	async function step(go: (() => unknown) | undefined) {
		await go?.();
		await tick();
		const active = document.activeElement;
		const lost =
			!active ||
			active === document.body ||
			(active instanceof HTMLButtonElement && active.disabled);
		if (lost) panel?.focus({ preventScroll: true });
	}
</script>

<Portal>
	{#if open}
		<div
			{@attach lifecycle}
			class="slide-over"
			class:phone={phone.current}
			role="dialog"
			aria-labelledby={titleId}
			tabindex="-1"
			style:--panel-width={width}
			{onkeydown}
		>
			<div class="head">
				<div class="bar">
					{#if phone.current}
						<IconButton label={ru.common.back} icon={ArrowLeft} onclick={close} />
					{/if}
					<h2 id={titleId}>{title}</h2>
					{#if onprev || onnext}
						<IconButton
							label={ru.panel.prev}
							icon={ChevronLeft}
							disabled={!onprev}
							onclick={() => step(onprev)}
						/>
						<IconButton
							label={ru.panel.next}
							icon={ChevronRight}
							disabled={!onnext}
							onclick={() => step(onnext)}
						/>
					{/if}
					{@render actions?.()}
					{#if !phone.current}
						<IconButton label={ru.common.close} icon={X} onclick={close} />
					{/if}
				</div>
				{@render header?.()}
			</div>
			<div class="body">{@render children()}</div>
			{#if footer}<div class="foot">{@render footer()}</div>{/if}
		</div>
	{/if}
</Portal>

<style>
	.slide-over {
		position: fixed;
		z-index: var(--z-panel);
		top: 0;
		right: 0;
		bottom: 0;
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		width: var(--panel-width);
		border-left: 1px solid var(--border);
		border-radius: var(--radius-lg) 0 0 var(--radius-lg);
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow-lg);
		outline: none;
		animation: slide-in var(--duration-slow) var(--ease-out);
	}
	.slide-over.phone {
		left: 0;
		width: auto;
		border: 0;
		border-radius: 0;
	}
	.head {
		flex: none;
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-3) var(--space-3) var(--space-5);
		border-bottom: 1px solid var(--border);
	}
	.phone .head {
		padding-left: var(--space-3);
	}
	.bar {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}
	h2 {
		flex: 1;
		min-width: 0;
		margin: 0;
		overflow: hidden;
		font-size: var(--text-lg);
		font-weight: 600;
		line-height: var(--leading-tight);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.body {
		flex: 1 1 auto;
		min-height: 0;
		overflow-y: auto;
		padding: var(--space-4) var(--space-5);
	}
	.foot {
		flex: none;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-5);
		border-top: 1px solid var(--border);
	}
	@keyframes slide-in {
		from {
			transform: translateX(100%);
		}
	}
</style>
