<!-- The fields of a position (decision 18): text is saved a moment after the last key or on leaving the
     field, choices at once. A conflict marks the disputed fields with «Оставить моё» / «Взять новое». -->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { StockItem } from '$api/StockItem';
	import { formatMoney } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import type { Autosave } from '#lib/orders/autosave.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import MoneyInput from '#lib/ui/MoneyInput.svelte';
	import QuantityInput from '#lib/ui/QuantityInput.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import type { StockConflict, StockEditField, StockValues } from './editor.svelte.js';
	import { MAX_STOCK_QUANTITY, STOCK_FIELDS } from './fields.js';

	let {
		item,
		autosave,
		dictionaries,
		readonly = false,
		errors = {},
		conflicts = new Map(),
		flashed = new Set(),
		onkeep,
		ontake,
		onflashend
	}: {
		item: StockItem;
		autosave: Autosave<StockValues>;
		/** The dictionaries behind the choices: by the field they fill. */
		dictionaries: Record<'part_type' | 'device_type' | 'brand' | 'color', string[]>;
		/** No editing (no right to edit the stock). */
		readonly?: boolean;
		/** What the server said is wrong with a field. */
		errors?: Partial<Record<string, string>>;
		conflicts?: ReadonlyMap<StockEditField, StockConflict>;
		flashed?: ReadonlySet<StockEditField>;
		onkeep?: (field: StockEditField) => void;
		ontake?: (field: StockEditField) => void;
		onflashend?: (field: StockEditField) => void;
	} = $props();

	const t = ru.stock;
	const o = ru.order;

	/** What the field shows: own unsaved value, or the position's. */
	const shown = <K extends StockEditField>(f: K): StockValues[K] => autosave.value(f, item[f]);
	const choices = (f: keyof typeof dictionaries) => dictionaries[f].map((value) => ({ value }));

	/** The name the server makes when the field stays empty: the hint of the field. */
	const derived = $derived(
		(['part_type', 'device_type', 'brand', 'model', 'color'] as const)
			.map((f) => shown(f).trim())
			.filter((v) => v !== '')
			.join(' ')
	);

	/** A price typed that is not an amount is not sent. */
	let priceInvalid = $state(false);

	/** Leaving a field saves what was typed in it; another window taking focus (Alt-Tab) is not leaving. */
	function leave(e: FocusEvent) {
		void autosave.flush().catch(() => {});
		if (e.relatedTarget === null || !document.hasFocus()) return;
		autosave.release();
	}

	function asText(f: StockEditField, v: unknown): string {
		if (v === null || v === '') return o.empty;
		return f === 'price' ? formatMoney(v as number) : String(v);
	}
</script>

{#snippet dispute(f: StockEditField)}
	{@const c = conflicts.get(f)}
	{#if c}
		<div class="dispute" role="group" aria-label={o.conflictField(STOCK_FIELDS[f])}>
			<p class="theirs"><span class="muted">{o.theirs}:</span> {asText(f, c.theirs)}</p>
			<div class="choices">
				<Button size="sm" onclick={() => onkeep?.(f)}>{o.keepMine}</Button>
				<Button size="sm" onclick={() => ontake?.(f)}>{o.takeNew}</Button>
			</div>
		</div>
	{/if}
{/snippet}

{#snippet flashable(f: StockEditField, body: Snippet)}
	<div class="flashable" class:flash={flashed.has(f)} onanimationend={() => onflashend?.(f)}>
		{@render body()}
		{@render dispute(f)}
	</div>
{/snippet}

<!-- Disputed fields are only decided by the buttons: typing would settle them. -->
{#snippet text(f: 'name' | 'model')}
	{#snippet body()}
		<TextInput
			id="stock-{f}"
			label={STOCK_FIELDS[f]}
			help={f === 'name' ? ru.help.stock.name : undefined}
			placeholder={f === 'name' ? derived : undefined}
			autocomplete="off"
			maxlength={200}
			disabled={readonly || conflicts.has(f)}
			error={errors[f]}
			bind:value={() => shown(f), (v) => autosave.edit(f, v)}
		/>
	{/snippet}
	{@render flashable(f, body)}
{/snippet}

{#snippet choice(f: keyof typeof dictionaries)}
	{#snippet body()}
		<Combobox
			id="stock-{f}"
			label={STOCK_FIELDS[f]}
			items={choices(f)}
			disabled={readonly || conflicts.has(f)}
			error={errors[f]}
			bind:value={() => shown(f), (v) => autosave.edit(f, v)}
			onselect={(v) => autosave.commit(f, v)}
		/>
	{/snippet}
	{@render flashable(f, body)}
{/snippet}

{#snippet count(f: 'quantity' | 'restock_threshold')}
	{#snippet body()}
		<QuantityInput
			id="stock-{f}"
			max={MAX_STOCK_QUANTITY}
			label={STOCK_FIELDS[f]}
			help={f === 'restock_threshold' ? ru.help.stock.threshold : undefined}
			disabled={readonly || conflicts.has(f)}
			error={errors[f]}
			value={shown(f)}
			onchange={(v) => autosave.edit(f, v)}
		/>
	{/snippet}
	{@render flashable(f, body)}
{/snippet}

<div class="fields" onfocusout={leave}>
	{#if conflicts.size > 0}
		<Callout tone="warning" title={t.conflictTitle}>{o.conflictText}</Callout>
	{/if}

	{@render text('name')}
	<div class="grid">
		{@render choice('part_type')}
		{@render choice('device_type')}
		{@render choice('brand')}
		{@render text('model')}
		{@render choice('color')}
	</div>
	<div class="grid">
		{@render count('quantity')}
		{#snippet price()}
			<MoneyInput
				id="stock-price"
				label={STOCK_FIELDS.price}
				help={ru.help.stock.price}
				nullable={false}
				disabled={readonly || conflicts.has('price')}
				error={errors.price}
				bind:invalid={priceInvalid}
				bind:value={
					() => shown('price') as number | null,
					(v) => {
						if (!priceInvalid && v !== null) autosave.edit('price', v);
					}
				}
			/>
		{/snippet}
		{@render flashable('price', price)}
		{@render count('restock_threshold')}
	</div>
	{#snippet note()}
		<Textarea
			id="stock-note"
			label={STOCK_FIELDS.note}
			minRows={2}
			maxlength={2000}
			disabled={readonly || conflicts.has('note')}
			error={errors.note}
			bind:value={() => shown('note'), (v) => autosave.edit('note', v)}
		/>
	{/snippet}
	{@render flashable('note', note)}
</div>

<style>
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	/* Fields side by side as far as the panel is wide: no width queries, the grid decides. */
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
		gap: var(--space-3);
	}
	.muted {
		color: var(--text-3);
	}
	.flashable {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
		border-radius: var(--radius-md);
	}
	/* Taken from another computer: a short highlight (spec 6.2). */
	.flash {
		animation: flash var(--duration-flash) var(--ease-out);
	}
	@keyframes flash {
		from {
			background: var(--warning-soft);
			box-shadow: 0 0 0 var(--space-1) var(--warning-soft);
		}
	}
	.dispute {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--warning-soft);
		color: var(--warning-text);
		font-size: var(--text-sm);
	}
	.theirs {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.choices {
		display: flex;
		gap: var(--space-2);
	}
</style>
