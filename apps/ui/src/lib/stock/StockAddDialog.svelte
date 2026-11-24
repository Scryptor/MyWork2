<!-- «Добавить позицию» (decision 18): the fields of a position; the name may stay empty (it is made of the
     other fields). The server's refusals stay at their fields. After it the panel of the new position opens. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { NewStockItem } from '$api/NewStockItem';
	import type { StockItem } from '$api/StockItem';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { errorMessage } from '#lib/api/client.js';
	import { createStockItem } from '#lib/api/stock.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import MoneyInput from '#lib/ui/MoneyInput.svelte';
	import QuantityInput from '#lib/ui/QuantityInput.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import { MAX_STOCK_QUANTITY, STOCK_FIELDS, stockFieldErrors } from './fields.js';
	import { stockCount } from './summary.svelte.js';

	let {
		open = $bindable(false),
		oncreated
	}: { open: boolean; oncreated: (item: StockItem) => void } = $props();

	const t = ru.stock;
	const empty = () => ({
		name: '',
		part_type: '',
		device_type: '',
		brand: '',
		model: '',
		color: '',
		note: '',
		quantity: 0,
		price: 0,
		restock_threshold: 0
	});
	let form = $state(empty());
	let errors = $state<Record<string, string>>({});
	let busy = $state(false);
	let priceInvalid = $state(false);
	let dictionaries = $state.raw({
		part_type: [] as string[],
		device_type: [] as string[],
		brand: [] as string[],
		color: [] as string[]
	});
	const KINDS = ['part_type', 'device_type', 'brand', 'color'] as const;
	const items = (values: string[]) => values.map((value) => ({ value }));

	// Every opening starts from an empty form and fresh dictionaries (the settings may have changed).
	$effect(() => {
		if (!open) return;
		untrack(() => {
			form = empty();
			errors = {};
			priceInvalid = false;
			let current = true;
			void Promise.all(KINDS.map((k) => listDictionary(k).catch(() => []))).then((lists) => {
				// Without them the fields still take any text.
				if (!current) return;
				dictionaries = Object.fromEntries(
					KINDS.map((k, i) => [k, (lists[i] ?? []).map((d) => d.value)])
				) as typeof dictionaries;
			});
			stopLoading = () => (current = false);
		});
		return () => stopLoading();
	});
	let stopLoading = () => {};

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});

	/** The name the server will make when this one stays empty: shown as the hint of the field. */
	const derived = $derived(
		[form.part_type, form.device_type, form.brand, form.model, form.color]
			.map((v) => v.trim())
			.filter((v) => v !== '')
			.join(' ')
	);

	async function submit() {
		if (busy || priceInvalid) return;
		errors = {};
		busy = true;
		try {
			const text = (v: string) => v.trim();
			const body: NewStockItem = {
				name: text(form.name),
				part_type: text(form.part_type),
				device_type: text(form.device_type),
				brand: text(form.brand),
				model: text(form.model),
				color: text(form.color),
				note: text(form.note),
				quantity: form.quantity,
				price: form.price,
				restock_threshold: form.restock_threshold
			};
			const item = await createStockItem(body);
			stockCount.refresh();
			open = false;
			oncreated(item);
		} catch (e) {
			errors = stockFieldErrors(e);
			// A refusal at a field is shown there; anything else is told.
			if (Object.keys(errors).length === 0) {
				toast.show({ message: t.addFailed(errorMessage(e)), tone: 'danger' });
			}
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open title={t.addTitle} dismissible={!busy}>
	<form
		class="form"
		onsubmit={(e) => {
			e.preventDefault();
			void submit();
		}}
	>
		<TextInput
			label={STOCK_FIELDS.name}
			hint={t.nameHint}
			placeholder={derived}
			autocomplete="off"
			maxlength={200}
			error={errors.name}
			bind:value={form.name}
		/>
		<div class="grid">
			<Combobox
				label={STOCK_FIELDS.part_type}
				items={items(dictionaries.part_type)}
				error={errors.part_type}
				bind:value={form.part_type}
			/>
			<Combobox
				label={STOCK_FIELDS.device_type}
				items={items(dictionaries.device_type)}
				error={errors.device_type}
				bind:value={form.device_type}
			/>
			<Combobox
				label={STOCK_FIELDS.brand}
				items={items(dictionaries.brand)}
				error={errors.brand}
				bind:value={form.brand}
			/>
			<TextInput
				label={STOCK_FIELDS.model}
				autocomplete="off"
				maxlength={200}
				error={errors.model}
				bind:value={form.model}
			/>
			<Combobox
				label={STOCK_FIELDS.color}
				items={items(dictionaries.color)}
				error={errors.color}
				bind:value={form.color}
			/>
		</div>
		<div class="grid">
			<QuantityInput
				max={MAX_STOCK_QUANTITY}
				label={STOCK_FIELDS.quantity}
				error={errors.quantity}
				bind:value={form.quantity}
			/>
			<MoneyInput
				label={STOCK_FIELDS.price}
				nullable={false}
				error={errors.price}
				bind:value={form.price as number | null}
				bind:invalid={priceInvalid}
			/>
			<QuantityInput
				label={STOCK_FIELDS.restock_threshold}
				max={MAX_STOCK_QUANTITY}
				help={ru.help.stock.threshold}
				error={errors.restock_threshold}
				bind:value={form.restock_threshold}
			/>
		</div>
		<Textarea
			label={STOCK_FIELDS.note}
			maxlength={2000}
			error={errors.note}
			bind:value={form.note}
		/>
		<div class="actions">
			<Button onclick={() => (open = false)} disabled={busy}>{ru.common.cancel}</Button>
			<Button type="submit" variant="primary" {busy}>{t.addSubmit}</Button>
		</div>
	</form>
</Dialog>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	/* Fields side by side as far as the dialog is wide: no width queries, the grid decides. */
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
		gap: var(--space-3);
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
