<!-- «Положить…» / «Переложить…» (plan 8.6): the cell and the quantity of a position; or, from the panel of a
     cell (`here`), the position to lay into that cell. The action is the caller's (`onsubmit`): the panel of
     a position runs it through its editor. A refused quantity (`places_exceed`) is explained by the position
     as it is now. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { StockCellRef } from '$api/StockCellRef';
	import type { StockItem } from '$api/StockItem';
	import type { StockPlace } from '$api/StockPlace';
	import type { StockRack } from '$api/StockRack';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { listStock } from '#lib/api/stock.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import QuantityInput from '#lib/ui/QuantityInput.svelte';
	import { cellOptions, exceedText, sameCell } from './places.js';

	let {
		open = $bindable(false),
		mode,
		racks,
		item,
		from,
		cell,
		onsubmit,
		reload
	}: {
		open: boolean;
		mode: 'put' | 'move' | 'here';
		racks: StockRack[];
		/** `put` and `move`: the position. */
		item?: StockItem;
		/** `move`: the place the units are taken from. */
		from?: StockPlace;
		/** `here`: the cell is given, the position is chosen. */
		cell?: { ref: StockCellRef; code: string };
		onsubmit: (itemId: number, ref: StockCellRef, quantity: number) => Promise<void>;
		/** The position as the server has it now: for the text of `places_exceed`. */
		reload: (itemId: number) => Promise<StockItem>;
	} = $props();

	const t = ru.stock.places;

	let cellText = $state('');
	let itemText = $state('');
	let chosen = $state.raw<StockItem | null>(null);
	let found = $state.raw<StockItem[]>([]);
	let quantity = $state(1);
	let busy = $state(false);
	let cellError = $state<string | undefined>();
	let itemError = $state<string | undefined>();
	let quantityError = $state<string | undefined>();
	let problem = $state<string | undefined>();

	const options = $derived(
		cellOptions(racks).filter((o) => !(mode === 'move' && from && sameCell(o.ref, from)))
	);
	const target = $derived(
		mode === 'here' ? (cell?.ref ?? null) : (options.find((o) => o.value === cellText)?.ref ?? null)
	);
	const subject = $derived(mode === 'here' ? chosen : (item ?? null));
	const available = $derived(mode === 'move' ? (from?.quantity ?? 0) : (subject?.unplaced ?? 0));
	const title = $derived(
		mode === 'put' ? t.putTitle : mode === 'move' ? t.moveTitle : t.hereTitle(cell?.code ?? '')
	);
	const submitLabel = $derived(mode === 'move' ? t.submitMove : t.submitPut);
	const label = (i: StockItem) => `${i.display_name} · ${ru.stock.article(i.id)}`;
	const itemItems = $derived(
		found.map((i) => ({ value: label(i), hint: t.itemUnplaced(i.unplaced) }))
	);

	// Every opening starts empty, with all that can be moved as the quantity.
	$effect(() => {
		if (!open) return;
		untrack(() => {
			cellText = '';
			itemText = '';
			chosen = null;
			found = [];
			cellError = itemError = quantityError = problem = undefined;
			quantity = Math.max(1, available);
		});
	});

	// `here`: the positions follow the typing a moment after the last key.
	$effect(() => {
		if (!open || mode !== 'here') return;
		const text = itemText.trim();
		let current = true;
		const timer = setTimeout(
			() => {
				listStock({ query: text || undefined, limit: 20, sort: 'name' })
					.then((page) => {
						if (current) found = page.items.filter((i) => i.unplaced > 0);
					})
					.catch(() => {
						if (current) problem = t.searchFailed;
					});
			},
			text === '' ? 0 : 250
		);
		return () => {
			current = false;
			clearTimeout(timer);
		};
	});

	function pickItem(value: string) {
		const i = found.find((f) => label(f) === value);
		if (!i) return;
		chosen = i;
		itemError = undefined;
		quantity = Math.max(1, i.unplaced);
	}

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});

	async function submit() {
		if (busy) return;
		cellError = itemError = quantityError = problem = undefined;
		if (!subject) {
			itemError = t.pickItem;
			return;
		}
		if (!target) {
			cellError = t.pickCell;
			return;
		}
		busy = true;
		try {
			await onsubmit(subject.id, target, quantity);
			open = false;
		} catch (e) {
			await explain(e, subject);
		} finally {
			busy = false;
		}
	}

	async function explain(e: unknown, subj: StockItem) {
		const fields = e instanceof ApiError && e.code === 'validation' ? (e.body?.fields ?? []) : [];
		if (fields.some((f) => f.field === 'quantity' && f.code === 'places_exceed')) {
			try {
				const fresh = await reload(subj.id);
				const now =
					mode === 'move' && from
						? (fresh.places.find((p) => sameCell(p, from)) ?? { ...from, quantity: 0 })
						: undefined;
				quantityError = exceedText(fresh, now);
				quantity = Math.max(1, Math.min(quantity, now ? now.quantity : fresh.unplaced));
			} catch (again) {
				problem = t.failed(errorMessage(again));
			}
			return;
		}
		const lost = fields.find((f) => f.code === 'no_such_place');
		if (lost) {
			cellError = lost.message || t.noSuchPlace;
			return;
		}
		problem = t.failed(errorMessage(e));
	}
</script>

<Dialog bind:open {title} dismissible={!busy}>
	<form
		class="form"
		onsubmit={(e) => {
			e.preventDefault();
			void submit();
		}}
	>
		{#if mode === 'here'}
			<Combobox
				label={t.item}
				placeholder={t.itemPlaceholder}
				items={itemItems}
				filter={false}
				error={itemError}
				bind:value={itemText}
				onselect={pickItem}
			/>
		{:else}
			<Combobox
				label={t.cell}
				placeholder={t.cellPlaceholder}
				items={options}
				allowCustom={false}
				error={cellError}
				bind:value={cellText}
			/>
		{/if}
		<QuantityInput
			label={t.quantity}
			min={1}
			max={Math.max(1, available)}
			error={quantityError}
			bind:value={quantity}
		/>
		{#if mode === 'here' && cellError}<p class="error" role="alert">{cellError}</p>{/if}
		{#if problem}<p class="error" role="alert">{problem}</p>{/if}
		<div class="actions">
			<Button onclick={() => (open = false)} disabled={busy}>{ru.common.cancel}</Button>
			<Button type="submit" variant="primary" {busy}>{submitLabel}</Button>
		</div>
	</form>
</Dialog>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
	.error {
		margin: 0;
		color: var(--danger-text);
	}
</style>
