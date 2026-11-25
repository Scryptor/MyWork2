<!-- «Добавить шкаф» / «Изменить…» (plan 8.6): code, label, the grid and the point of a rack. Only what changed
     is sent. A smaller grid over parts is refused by the server (409): its text stays in the dialog. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { NewStockRack } from '$api/NewStockRack';
	import type { ServicePoint } from '$api/ServicePoint';
	import type { StockRack } from '$api/StockRack';
	import type { StockRackPatch } from '$api/StockRackPatch';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { createStockRack, updateStockRack } from '#lib/api/stock.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import QuantityInput from '#lib/ui/QuantityInput.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { codesWillChange, NO_POINT } from './racks.js';

	let {
		open = $bindable(false),
		rack,
		points,
		onsaved
	}: {
		open: boolean;
		/** The rack to change; none — a new one. */
		rack?: StockRack;
		/** The points that can be chosen: not archived. */
		points: ServicePoint[];
		onsaved: (rack: StockRack) => void;
	} = $props();

	const t = ru.stock.map.rack;
	const MAX_SIDE = 20;

	let code = $state('');
	let label = $state('');
	let shelves = $state(1);
	let perShelf = $state(1);
	let point = $state<string | null>(NO_POINT);
	let busy = $state(false);
	let errors = $state<Record<string, string>>({});
	let problem = $state<string | undefined>();

	const pointValue = (id: number | null | undefined) => (id == null ? NO_POINT : String(id));

	// Every opening starts from the rack as it is now.
	$effect(() => {
		if (!open) return;
		untrack(() => {
			code = rack?.code ?? '';
			label = rack?.label ?? '';
			shelves = rack?.shelves ?? 1;
			perShelf = rack?.cells_per_shelf ?? 1;
			point = pointValue(rack?.service_point_id);
			errors = {};
			problem = undefined;
		});
	});
	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});

	/** The points to choose from; the rack's own stays even when it has been archived since. */
	const pointItems = $derived.by(() => {
		const items = points.map((p) => ({ value: String(p.id), label: p.name }));
		const own = rack?.service_point_id;
		if (own != null && !items.some((i) => i.value === String(own))) {
			items.push({ value: String(own), label: `№ ${own}` });
		}
		return [{ value: NO_POINT, label: ru.stock.map.noPoint }, ...items];
	});

	const warn = $derived(
		rack !== undefined && codesWillChange(rack, { code, shelves, cells_per_shelf: perShelf })
	);

	/** The fields that differ from the rack; for a new rack — all of them. */
	function changes(): StockRackPatch {
		const next = point === NO_POINT || point === null ? null : Number(point);
		const out: StockRackPatch = {};
		if (!rack || code.trim() !== rack.code) out.code = code.trim();
		if (!rack || label.trim() !== rack.label) out.label = label.trim();
		if (!rack || shelves !== rack.shelves) out.shelves = shelves;
		if (!rack || perShelf !== rack.cells_per_shelf) out.cells_per_shelf = perShelf;
		if (!rack || next !== rack.service_point_id) out.service_point_id = next;
		return out;
	}

	async function submit() {
		if (busy) return;
		errors = {};
		problem = undefined;
		const patch = changes();
		if (rack && Object.keys(patch).length === 0) {
			open = false;
			return;
		}
		busy = true;
		try {
			const saved = rack
				? await updateStockRack(rack.id, patch)
				: await createStockRack(patch as NewStockRack);
			open = false;
			onsaved(saved);
		} catch (e) {
			explain(e);
		} finally {
			busy = false;
		}
	}

	function explain(e: unknown) {
		if (e instanceof ApiError && e.code === 'validation') {
			const fields = e.body?.fields ?? [];
			if (fields.length > 0) {
				errors = Object.fromEntries(fields.map((f) => [f.field, t.errors[f.code] ?? f.message]));
				return;
			}
		}
		problem =
			e instanceof ApiError && e.code === 'conflict' ? e.message : t.failed(errorMessage(e));
	}
</script>

<Dialog bind:open title={rack ? t.editTitle(rack.code) : t.addTitle} dismissible={!busy}>
	<form
		class="form"
		onsubmit={(e) => {
			e.preventDefault();
			void submit();
		}}
	>
		<TextInput
			label={t.code}
			hint={t.codeHint}
			autocomplete="off"
			maxlength={8}
			error={errors.code}
			bind:value={code}
		/>
		<TextInput
			label={t.label}
			autocomplete="off"
			maxlength={100}
			error={errors.label}
			bind:value={label}
		/>
		<div class="grid">
			<QuantityInput
				label={t.shelves}
				min={1}
				max={MAX_SIDE}
				error={errors.shelves}
				bind:value={shelves}
			/>
			<QuantityInput
				label={t.perShelf}
				min={1}
				max={MAX_SIDE}
				error={errors.cells_per_shelf}
				bind:value={perShelf}
			/>
		</div>
		<Select label={t.point} items={pointItems} error={errors.service_point_id} bind:value={point} />
		{#if warn}<Callout tone="warning">{t.codesChange}</Callout>{/if}
		{#if problem}<Callout tone="danger">{problem}</Callout>{/if}
		<div class="actions">
			<Button onclick={() => (open = false)} disabled={busy}>{ru.common.cancel}</Button>
			<Button type="submit" variant="primary" {busy}>{rack ? t.save : t.add}</Button>
		</div>
	</form>
</Dialog>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
		gap: var(--space-3);
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
