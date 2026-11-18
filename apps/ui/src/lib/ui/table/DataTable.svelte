<script lang="ts" generics="Row">
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import { createVirtualizer } from '@tanstack/svelte-virtual';
	import { untrack, type Snippet } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { display, prefs } from '#lib/design/prefs.svelte.js';
	import { ROW_REM } from './sizes.js';
	import { ru } from '#lib/i18n/ru.js';
	import EmptyState from '../EmptyState.svelte';
	import Skeleton from '../Skeleton.svelte';
	import ColumnsMenu from './ColumnsMenu.svelte';
	import RowCheck from './RowCheck.svelte';
	import { clampColumnWidth, loadColumnPrefs, saveColumnPrefs } from './column-prefs.js';
	import { rangeSelect } from './selection.js';
	import type { Column, ColumnPrefs, RowId, SortState } from './types.js';

	let {
		rows,
		columns,
		rowId,
		storageKey,
		ariaLabel,
		sort = null,
		onsortchange,
		selectable = false,
		selected = $bindable(new Set<RowId>()),
		activeId = $bindable(null),
		onopen,
		loading = false,
		empty,
		rowClass,
		onendreached,
		flashIds
	}: {
		rows: Row[];
		columns: Column<Row>[];
		rowId: (r: Row) => RowId;
		/** Column widths, order and visibility are kept on this device under this key. */
		storageKey: string;
		ariaLabel: string;
		/** The table only reports a header click (`onsortchange`): the rows come sorted from outside. */
		sort?: SortState | null;
		onsortchange?: (s: SortState) => void;
		selectable?: boolean;
		selected?: Set<RowId>;
		/** The row under the keyboard cursor, the one open in a panel (`aria-current`, not a selection). */
		activeId?: RowId | null;
		onopen?: (r: Row) => void;
		loading?: boolean;
		empty?: Snippet;
		rowClass?: (r: Row) => string | undefined;
		/** Asked when the last visible row is within 20 of the end of `rows`, once per `rows` array. */
		onendreached?: () => void;
		/** A new array briefly highlights these rows (changed on another computer). */
		flashIds?: RowId[];
	} = $props();

	const uid = $props.id();

	// Sizes follow the UI scale: whole-pixel row height by density (for the virtualizer and the DOM
	// alike), column widths (stored for the normal size) scaled like the text.
	const scale = $derived(display.value.scale);
	const rowPx = $derived(Math.round(ROW_REM[prefs.value.density] * 16 * scale));

	// Columns: what this device changed, saved at once.
	let colPrefs = $derived(loadColumnPrefs(storageKey, columns));
	function commit(next: ColumnPrefs) {
		colPrefs = next;
		saveColumnPrefs(storageKey, next);
	}
	const byId = $derived(new Map(columns.map((c) => [c.id, c])));
	const visible = $derived(
		colPrefs.order.flatMap((id) => {
			const c = byId.get(id);
			return c && !colPrefs.hidden.includes(id) ? [c] : [];
		})
	);
	let resizing = $state<{ id: string; width: number } | null>(null);
	function widthOf(c: Column<Row>): number {
		return resizing?.id === c.id ? resizing.width : (colPrefs.widths[c.id] ?? c.width);
	}
	const shownWidths = $derived(visible.map((c) => Math.round(widthOf(c) * scale)));
	const template = $derived(
		[
			...(selectable ? ['var(--control-lg)'] : []),
			...shownWidths.map((w) => `${w}px`),
			'minmax(var(--control-lg), 1fr)'
		].join(' ')
	);
	const minWidth = $derived(
		`calc(${shownWidths.reduce((sum, w) => sum + w, 0)}px + ${selectable ? 2 : 1} * var(--control-lg))`
	);

	// Virtual rows. The sticky header is inside the scroller, one row high: rows start below it.
	let scroller = $state<HTMLDivElement | null>(null);
	const skeletons = $derived(loading ? (rows.length === 0 ? 8 : 3) : 0);
	const virtualizer = createVirtualizer<HTMLDivElement, HTMLDivElement>({
		count: 0,
		getScrollElement: () => null,
		estimateSize: () => 40,
		overscan: 10
	});
	let measuredPx = 0;
	$effect(() => {
		const size = rowPx;
		const el = scroller;
		const list = rows;
		const count = list.length + skeletons;
		untrack(() => {
			$virtualizer.setOptions({
				count,
				getScrollElement: () => el,
				estimateSize: () => size,
				scrollMargin: size,
				scrollPaddingStart: size,
				getItemKey: (i) => {
					const row = list[i];
					return row === undefined ? `skeleton-${i}` : rowId(row);
				}
			});
			if (size !== measuredPx) {
				measuredPx = size;
				$virtualizer.measure();
			}
		});
	});
	const items = $derived($virtualizer.getVirtualItems());
	const totalSize = $derived($virtualizer.getTotalSize());

	const indexById = $derived(new Map(rows.map((r, i) => [rowId(r), i])));
	const activeIndex = $derived(activeId === null ? -1 : (indexById.get(activeId) ?? -1));
	const activeRendered = $derived(activeIndex >= 0 && items.some((it) => it.index === activeIndex));
	const rowDomId = (index: number) => `${uid}-row-${index}`;

	// The active row stays in view, whoever moved it.
	$effect(() => {
		const id = activeId;
		if (id === null) return;
		untrack(() => {
			const index = indexById.get(id);
			if (index !== undefined) $virtualizer.scrollToIndex(index, { align: 'auto' });
		});
	});

	// Once per `rows` array: a page more or a list sorted anew may ask again, scrolling may not.
	let requestedFor: Row[] | null = null;
	$effect(() => {
		const v = $virtualizer;
		const list = rows;
		if (loading || list.length === 0 || !onendreached || requestedFor === list) return;
		v.getVirtualItems();
		const last = v.range?.endIndex ?? -1;
		if (last >= 0 && last >= list.length - 20) {
			requestedFor = list;
			untrack(() => onendreached?.());
		}
	});

	// Flash: an alternating tag restarts the animation when the same row flashes again.
	const FLASH_MS = 1200; // --duration-flash
	const flashing = new SvelteMap<RowId, number>();
	let flashGen = 0;
	const flashTimers: ReturnType<typeof setTimeout>[] = [];
	$effect(() => {
		const ids = flashIds ?? [];
		if (ids.length === 0) return;
		untrack(() => {
			const gen = ++flashGen;
			for (const id of ids) flashing.set(id, gen);
			const timer = setTimeout(() => {
				flashTimers.splice(flashTimers.indexOf(timer), 1);
				for (const id of ids) if (flashing.get(id) === gen) flashing.delete(id);
			}, FLASH_MS);
			flashTimers.push(timer);
		});
	});
	$effect(() => () => flashTimers.forEach((t) => clearTimeout(t)));

	function sortOf(c: Column<Row>): 'ascending' | 'descending' | undefined {
		if (sort?.columnId !== c.id) return undefined;
		return sort.descending ? 'descending' : 'ascending';
	}
	function toggleSort(c: Column<Row>) {
		onsortchange?.({
			columnId: c.id,
			descending: sort?.columnId === c.id ? !sort.descending : true
		});
	}

	// Selection: checkboxes, Shift+click ranges from the anchor (the row clicked or toggled last).
	let anchor: RowId | null = null;
	const selectedHere = $derived(rows.reduce((n, r) => (selected.has(rowId(r)) ? n + 1 : n), 0));
	const allSelected = $derived(rows.length > 0 && selectedHere === rows.length);

	function toggle(id: RowId) {
		selected = selected.has(id)
			? new Set([...selected].filter((x) => x !== id))
			: new Set([...selected, id]);
		anchor = id;
	}
	function extendTo(id: RowId) {
		selected = new Set([...selected, ...rangeSelect(rows.map(rowId), anchor ?? id, id)]);
	}
	function onSelectAll() {
		const here = new Set(rows.map(rowId));
		selected = allSelected
			? new Set([...selected].filter((id) => !here.has(id)))
			: new Set([...selected, ...here]);
	}
	function onRowCheck(e: MouseEvent, index: number) {
		e.stopPropagation();
		const row = rows[index];
		if (row === undefined) return;
		const id = rowId(row);
		if (e.shiftKey) extendTo(id);
		else toggle(id);
		scroller?.focus({ preventScroll: true });
	}

	function rowIndexOf(target: EventTarget | null): number {
		if (!(target instanceof Element)) return -1;
		const el = target.closest<HTMLElement>('[data-index]');
		return el && scroller?.contains(el) ? Number(el.dataset.index) : -1;
	}
	function onGridMouseDown(e: MouseEvent) {
		// Shift+click on a row selects rows, not text.
		if (e.shiftKey && rowIndexOf(e.target) >= 0) {
			e.preventDefault();
			scroller?.focus({ preventScroll: true });
		}
	}
	function onGridClick(e: MouseEvent) {
		const index = rowIndexOf(e.target);
		const row = index >= 0 ? rows[index] : undefined;
		if (row === undefined) return;
		if (
			e.target instanceof Element &&
			e.target.closest('a, button, input, select, textarea, label')
		) {
			return;
		}
		scroller?.focus({ preventScroll: true });
		const id = rowId(row);
		if (e.shiftKey && selectable) {
			extendTo(id);
			return;
		}
		activeId = id;
		anchor = id;
		onopen?.(row);
	}

	function pageSize(): number {
		return Math.max(1, Math.floor((scroller?.clientHeight ?? 0) / rowPx) - 2);
	}
	function moveTo(index: number) {
		const row = rows[index];
		if (row === undefined) return;
		activeId = rowId(row);
		$virtualizer.scrollToIndex(index, { align: 'auto' });
	}
	function onKeydown(e: KeyboardEvent) {
		if (e.target !== scroller || e.altKey || e.ctrlKey || e.metaKey) return;
		const n = rows.length;
		if (n === 0) return;
		const at = activeIndex;
		const current = at >= 0 ? rows[at] : undefined;
		let next: number;
		switch (e.key) {
			case 'ArrowDown':
				next = at < 0 ? 0 : Math.min(n - 1, at + 1);
				break;
			case 'ArrowUp':
				next = Math.max(0, at - 1);
				break;
			case 'PageDown':
				next = at < 0 ? 0 : Math.min(n - 1, at + pageSize());
				break;
			case 'PageUp':
				next = Math.max(0, at - pageSize());
				break;
			case 'Home':
				next = 0;
				break;
			case 'End':
				next = n - 1;
				break;
			case 'Enter':
				if (current === undefined) return;
				e.preventDefault();
				anchor = rowId(current);
				onopen?.(current);
				return;
			case ' ':
				if (!selectable || current === undefined) return;
				e.preventDefault();
				toggle(rowId(current));
				return;
			default:
				return;
		}
		e.preventDefault();
		moveTo(next);
	}

	// Width: drag the header edge, double click returns the default.
	function startResize(e: PointerEvent, c: Column<Row>) {
		if (e.button !== 0) return;
		e.preventDefault();
		const handle = e.currentTarget;
		if (handle instanceof HTMLElement) {
			try {
				handle.setPointerCapture(e.pointerId);
			} catch {
				// capture is a nicety: the window listeners below do the work
			}
		}
		const startX = e.clientX;
		const startWidth = widthOf(c);
		const k = scale;
		resizing = { id: c.id, width: startWidth };
		const move = (ev: PointerEvent) => {
			resizing = {
				id: c.id,
				width: clampColumnWidth(startWidth + (ev.clientX - startX) / k, c.minWidth)
			};
		};
		const end = () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', end);
			window.removeEventListener('pointercancel', end);
			const done = resizing;
			resizing = null;
			if (done && done.width !== startWidth) {
				commit({ ...colPrefs, widths: { ...colPrefs.widths, [c.id]: done.width } });
			}
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', end);
		window.addEventListener('pointercancel', end);
	}
	function resetWidth(c: Column<Row>) {
		commit({
			...colPrefs,
			widths: { ...colPrefs.widths, [c.id]: clampColumnWidth(c.width, c.minWidth) }
		});
	}
</script>

<div class="mw-table" style:--cols={template} style:--table-min={minWidth}>
	<div
		bind:this={scroller}
		class="scroller"
		role="grid"
		tabindex="0"
		aria-label={ariaLabel}
		aria-multiselectable={selectable ? 'true' : undefined}
		aria-rowcount={rows.length + 1}
		aria-busy={loading ? 'true' : undefined}
		aria-activedescendant={activeRendered ? rowDomId(activeIndex) : undefined}
		onkeydown={onKeydown}
		onmousedown={onGridMouseDown}
		onclick={onGridClick}
	>
		<div class="head" role="rowgroup" style:height="{rowPx}px">
			<div class="hrow" role="row" aria-rowindex={1}>
				{#if selectable}
					<div class="cell check" role="columnheader">
						<RowCheck
							checked={allSelected}
							indeterminate={!allSelected && selectedHere > 0}
							label={ru.table.selectAll}
							onclick={onSelectAll}
						/>
					</div>
				{/if}
				{#each visible as c (c.id)}
					{@const order = sortOf(c)}
					<div class="cell hcell {c.align ?? 'start'}" role="columnheader" aria-sort={order}>
						{#if c.sortable}
							<button type="button" class="sort" onclick={() => toggleSort(c)}>
								<span class="text">{c.header}</span>
								{#if order === 'descending'}
									<ArrowDown size={14} aria-hidden="true" />
								{:else if order === 'ascending'}
									<ArrowUp size={14} aria-hidden="true" />
								{/if}
							</button>
						{:else}
							<span class="text">{c.header}</span>
						{/if}
						<!-- Mouse and touch only: widths also reset by double click; the keyboard reorders and
						     hides columns in «Колонки». -->
						<span
							class="resize"
							class:dragging={resizing?.id === c.id}
							aria-hidden="true"
							onpointerdown={(e) => startResize(e, c)}
							ondblclick={() => resetWidth(c)}
						></span>
					</div>
				{/each}
				<div class="cell tail" role="columnheader">
					<ColumnsMenu {columns} bind:prefs={() => colPrefs, commit} />
				</div>
			</div>
		</div>
		<div
			class="body"
			role="rowgroup"
			style:height={rows.length > 0 || loading ? `${totalSize}px` : undefined}
		>
			{#if rows.length === 0 && !loading}
				<div class="empty" role="row" aria-rowindex={2}>
					<div role="gridcell">
						{#if empty}{@render empty()}{:else}<EmptyState title={ru.table.empty} />{/if}
					</div>
				</div>
			{:else}
				{#each items as item (item.key)}
					{@const row = rows[item.index]}
					{#if row !== undefined}
						{@const id = rowId(row)}
						{@const active = id === activeId}
						{@const flash = flashing.get(id)}
						<!-- aria-current: the cursor row; aria-selected: the checked rows (selectable tables only). -->
						<div
							id={rowDomId(item.index)}
							class={['row', rowClass?.(row), { active, checked: selected.has(id) }]}
							role="row"
							aria-rowindex={item.index + 2}
							aria-current={active ? 'true' : undefined}
							aria-selected={selectable ? selected.has(id) : undefined}
							data-index={item.index}
							data-flash={flash === undefined ? undefined : flash % 2 ? 'a' : 'b'}
							style:height="{item.size}px"
							style:transform="translateY({item.start - rowPx}px)"
						>
							{#if selectable}
								<div class="cell check" role="gridcell">
									<RowCheck
										checked={selected.has(id)}
										label={ru.table.selectRow}
										tabindex={-1}
										onclick={(e) => onRowCheck(e, item.index)}
									/>
								</div>
							{/if}
							{#each visible as c (c.id)}
								<div class="cell {c.align ?? 'start'}" role="gridcell">
									<span class="text">{@render c.cell(row)}</span>
								</div>
							{/each}
							<div class="cell tail" aria-hidden="true"></div>
						</div>
					{:else}
						<div
							class="row skeleton"
							aria-hidden="true"
							style:height="{item.size}px"
							style:transform="translateY({item.start - rowPx}px)"
						>
							{#if selectable}<div class="cell check"></div>{/if}
							{#each visible as c (c.id)}
								<div class="cell"><Skeleton width="70%" height="0.75rem" /></div>
							{/each}
							<div class="cell tail"></div>
						</div>
					{/if}
				{/each}
			{/if}
		</div>
	</div>
</div>

<style>
	.mw-table {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		border-radius: inherit;
		background: var(--surface);
		color: var(--text);
		font-size: var(--text-md);
		line-height: var(--leading-tight);
		font-variant-numeric: tabular-nums;
	}
	.scroller {
		position: relative;
		flex: 1;
		min-height: 0;
		overflow: auto;
		overscroll-behavior: contain;
	}
	/* The focus ring is drawn over the sticky header, which would cover the scroller's own outline. */
	.scroller:focus-visible {
		outline: none;
	}
	.mw-table:has(> .scroller:focus-visible)::after {
		content: '';
		position: absolute;
		inset: 0;
		z-index: calc(var(--z-sticky) + 1);
		border: 2px solid var(--focus);
		border-radius: inherit;
		pointer-events: none;
	}
	.head {
		position: sticky;
		top: 0;
		z-index: var(--z-sticky);
		min-width: var(--table-min);
		background: var(--surface-2);
		user-select: none;
	}
	.hrow,
	.row {
		display: grid;
		grid-template-columns: var(--cols);
		align-items: stretch;
		box-sizing: border-box;
		min-width: var(--table-min);
	}
	.hrow {
		height: 100%;
		border-bottom: 1px solid var(--border-strong);
		color: var(--text-2);
		font-size: var(--text-sm);
		font-weight: 600;
	}
	.body {
		position: relative;
		min-width: var(--table-min);
	}
	.row {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		border-bottom: 1px solid var(--border);
	}
	.row:hover {
		background: var(--surface-3);
	}
	.row.checked {
		background: var(--accent-soft);
	}
	.row.active {
		background: var(--accent-soft);
		box-shadow: inset 3px 0 0 var(--accent);
	}
	.row.skeleton:hover {
		background: none;
	}
	.row[data-flash='a'] {
		animation: flash-a var(--duration-flash) var(--ease-out);
	}
	.row[data-flash='b'] {
		animation: flash-b var(--duration-flash) var(--ease-out);
	}
	@keyframes flash-a {
		from {
			background: var(--accent-soft);
		}
	}
	@keyframes flash-b {
		from {
			background: var(--accent-soft);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.row[data-flash] {
			animation: none;
		}
	}
	.cell {
		display: flex;
		align-items: center;
		min-width: 0;
		box-sizing: border-box;
		padding: 0 var(--space-3);
		overflow: hidden;
	}
	.cell.end {
		justify-content: flex-end;
		text-align: end;
	}
	.cell.center {
		justify-content: center;
		text-align: center;
	}
	.cell.check {
		justify-content: center;
		padding: 0;
	}
	.cell.tail {
		padding: 0;
	}
	.text {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.hcell {
		position: relative;
		padding: 0;
	}
	.hcell > .text {
		padding: 0 var(--space-3);
	}
	.sort {
		display: flex;
		align-items: center;
		justify-content: inherit;
		gap: var(--space-1);
		width: 100%;
		height: 100%;
		min-width: 0;
		padding: 0 var(--space-3);
		border: 0;
		background: none;
		color: inherit;
		font: inherit;
		text-align: inherit;
		cursor: pointer;
	}
	.sort:hover {
		background: var(--surface-3);
		color: var(--text);
	}
	.sort:focus-visible {
		outline-offset: -2px;
	}
	.hcell[aria-sort] .sort {
		color: var(--text);
	}
	.resize {
		position: absolute;
		top: 0;
		right: 0;
		bottom: 0;
		z-index: 1;
		width: var(--space-2);
		cursor: col-resize;
		touch-action: none;
	}
	.resize::after {
		content: '';
		position: absolute;
		top: 25%;
		right: 0;
		bottom: 25%;
		width: 1px;
		background: var(--border-strong);
	}
	.resize:hover::after,
	.resize.dragging::after {
		top: 0;
		bottom: 0;
		width: 2px;
		background: var(--accent);
	}
	.hrow .tail {
		position: sticky;
		right: 0;
		justify-content: flex-end;
		padding: 0 var(--space-1);
		background: var(--surface-2);
	}
	.empty {
		min-width: var(--table-min);
	}
</style>
