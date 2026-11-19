<script lang="ts" generics="T">
	import GripVertical from '@lucide/svelte/icons/grip-vertical';
	import type { Snippet } from 'svelte';
	import { onDestroy, tick } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';

	let {
		items,
		key,
		label,
		row,
		onreorder,
		disabled = false
	}: {
		items: T[];
		key: (item: T) => number;
		label: (item: T) => string;
		row: Snippet<[T]>;
		onreorder: (ids: number[]) => void;
		disabled?: boolean;
	} = $props();

	let announcement = $state('');
	let list: HTMLElement | undefined = $state();
	/** The row being dragged, how far it is from its place, and where it would land. */
	let drag = $state<{ id: number; dy: number; to: number } | null>(null);

	/** Ends the drag in progress without a drop (the list goes away in the middle of it). */
	let abortDrag: (() => void) | null = null;
	onDestroy(() => abortDrag?.());

	const ids = $derived(items.map(key));

	function moved(order: number[], id: number, to: number): number[] {
		const rest = order.filter((x) => x !== id);
		rest.splice(to, 0, id);
		return rest;
	}

	/** Where the drop line goes: next to the row that the dragged one would take the place of. */
	function marker(id: number): 'before' | 'after' | '' {
		if (!drag || drag.id === id) return '';
		const others = ids.filter((x) => x !== drag!.id);
		const at = others.indexOf(id);
		const from = ids.indexOf(drag.id);
		if (drag.to === from) return '';
		if (drag.to <= from && at === drag.to) return 'before';
		if (drag.to > from && at === drag.to - 1) return 'after';
		return '';
	}

	async function focusHandle(id: number) {
		await tick();
		list?.querySelector<HTMLElement>(`[data-handle="${id}"]`)?.focus();
	}

	function onkey(e: KeyboardEvent, item: T) {
		const step = e.key === 'ArrowUp' ? -1 : e.key === 'ArrowDown' ? 1 : 0;
		if (!step) return;
		e.preventDefault();
		const id = key(item);
		const from = ids.indexOf(id);
		const to = from + step;
		if (to < 0 || to >= ids.length) return;
		announcement = ru.sortable.place(label(item), to + 1, ids.length);
		onreorder(moved(ids, id, to));
		void focusHandle(id);
	}

	function rows(): HTMLElement[] {
		return list ? [...list.querySelectorAll<HTMLElement>(':scope > li')] : [];
	}

	function target(centerY: number, id: number): number {
		// Index among the other rows: how many of them have their middle above the pointer.
		const others = rows().filter((_, i) => ids[i] !== id);
		let to = 0;
		for (const el of others) {
			const r = el.getBoundingClientRect();
			if (centerY > r.top + r.height / 2) to++;
		}
		return to;
	}

	function ondown(e: PointerEvent, item: T) {
		if (e.button !== 0) return;
		e.preventDefault();
		const id = key(item);
		const handle = e.currentTarget as HTMLElement;
		try {
			handle.setPointerCapture(e.pointerId);
		} catch {
			// capture is a nicety: the window listeners below do the work
		}
		const startY = e.clientY;
		const own = rows()[ids.indexOf(id)];
		const rect = own?.getBoundingClientRect();
		const half = rect ? rect.height / 2 : 0;
		const startCenter = rect ? rect.top + half : startY;
		drag = { id, dy: 0, to: ids.indexOf(id) };
		const move = (ev: PointerEvent) => {
			const dy = ev.clientY - startY;
			drag = { id, dy, to: target(startCenter + dy, id) };
		};
		const finish = (commit: boolean) => {
			abortDrag = null;
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', up);
			window.removeEventListener('pointercancel', cancel);
			const d = drag;
			drag = null;
			if (!commit || !d || d.to === ids.indexOf(id)) return;
			announcement = ru.sortable.place(label(item), d.to + 1, ids.length);
			onreorder(moved(ids, id, d.to));
		};
		abortDrag = () => finish(false);
		const up = () => finish(true);
		const cancel = () => finish(false);
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', up);
		window.addEventListener('pointercancel', cancel);
	}
</script>

<ul class="list" bind:this={list}>
	{#each items as item (key(item))}
		{@const id = key(item)}
		{@const dragging = drag?.id === id}
		<li
			class:dragging
			class:before={marker(id) === 'before'}
			class:after={marker(id) === 'after'}
			style:transform={dragging && drag ? `translateY(${drag.dy}px)` : undefined}
		>
			{#if !disabled}
				<button
					type="button"
					class="handle"
					data-handle={id}
					aria-label={ru.sortable.move(label(item))}
					onkeydown={(e) => onkey(e, item)}
					onpointerdown={(e) => ondown(e, item)}
				>
					<GripVertical size={16} aria-hidden="true" />
				</button>
			{/if}
			<div class="body">{@render row(item)}</div>
		</li>
	{/each}
</ul>
<div class="live" aria-live="polite" role="status">{announcement}</div>

<style>
	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	li {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
	}
	li.dragging {
		z-index: 1;
		box-shadow: var(--shadow-md);
		background: var(--surface-2);
		cursor: grabbing;
	}
	li.before::before,
	li.after::after {
		content: '';
		position: absolute;
		left: 0;
		right: 0;
		height: 2px;
		background: var(--accent);
	}
	li.before::before {
		top: -3px;
	}
	li.after::after {
		bottom: -3px;
	}
	.body {
		flex: 1;
		min-width: 0;
	}
	.handle {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--control-sm);
		height: var(--control-sm);
		padding: 0;
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text-2);
		cursor: grab;
		touch-action: none;
	}
	.handle:hover {
		background: var(--surface-3);
	}
	.handle:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 1px;
	}
	@media (pointer: coarse) {
		.handle {
			width: 2.75rem;
			height: 2.75rem;
		}
	}
	.live {
		position: absolute;
		width: 1px;
		height: 1px;
		margin: -1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
</style>
