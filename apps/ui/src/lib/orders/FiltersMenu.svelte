<!-- «Фильтры ▾» of the orders list (spec 6.1): type, brand, master, point, statuses, periods, flags.
     The choice applies with «Применить»; «Сохранить вид…» keeps the whole view in «Мои виды». -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ListFilter from '@lucide/svelte/icons/list-filter';
	import { tick, untrack } from 'svelte';
	import type { OrderValueField } from '$api/OrderValueField';
	import { listMasters } from '#lib/api/masters.js';
	import { orderValues } from '#lib/api/orders.js';
	import { app } from '#lib/app/context.svelte.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Checkbox from '#lib/ui/Checkbox.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import DateInput from '#lib/ui/DateInput.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Popover from '#lib/ui/Popover.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { filterMasterItems, type MasterList } from './masters.js';
	import type { OrderFilters, Period } from './view.js';

	let {
		filters,
		onapply,
		onsaveview,
		compact = false
	}: {
		filters: OrderFilters;
		onapply: (f: OrderFilters) => void;
		/** The name and the filters of the view, as applied when «Сохранить вид…» was pressed. */
		onsaveview: (name: string, filters: OrderFilters) => void;
		/** An icon button (a phone, next to the list filter). */
		compact?: boolean;
	} = $props();

	const t = ru.orders;
	const SUGGEST_MS = 150;
	/** «Как в верхней панели»; point ids start at 1. */
	const TOP_BAR_POINT = 0;
	/** «Любой» master, and the name of an old saved view; master ids start at 1. */
	const ANY_MASTER = 0;
	const BY_NAME = -1;

	interface Draft {
		deviceType: string;
		brand: string;
		/** A master user, or the name of an old saved view. */
		masterId: number | undefined;
		master: string;
		pointId: number;
		statusIds: number[];
		received: Period;
		issued: Period;
		needCall: boolean | undefined;
		needParts: boolean | undefined;
	}

	const toDraft = (f: OrderFilters): Draft => ({
		deviceType: f.deviceType ?? '',
		brand: f.brand ?? '',
		masterId: f.masterId,
		master: f.master ?? '',
		pointId: f.pointId ?? TOP_BAR_POINT,
		statusIds: [...(f.statusIds ?? [])],
		received: f.received ? [...f.received] : [null, null],
		issued: f.issued ? [...f.issued] : [null, null],
		needCall: f.needCall,
		needParts: f.needParts
	});
	const period = (p: Period): Period | undefined =>
		p[0] === null && p[1] === null ? undefined : [p[0], p[1]];
	function fromDraft(d: Draft): OrderFilters {
		const f: OrderFilters = {
			deviceType: d.deviceType.trim() || undefined,
			brand: d.brand.trim() || undefined,
			masterId: d.masterId,
			master: d.masterId === undefined ? d.master.trim() || undefined : undefined,
			pointId: d.pointId === TOP_BAR_POINT ? undefined : d.pointId,
			statusIds: d.statusIds.length > 0 ? [...d.statusIds] : undefined,
			received: period(d.received),
			issued: period(d.issued),
			needCall: d.needCall,
			needParts: d.needParts
		};
		return Object.fromEntries(Object.entries(f).filter(([, v]) => v !== undefined)) as OrderFilters;
	}

	let open = $state(false);
	let draft = $state<Draft>(toDraft({}));
	const invalid = $state({ rf: false, rt: false, if: false, it: false });
	const hasInvalid = $derived(Object.values(invalid).some(Boolean));

	// Every opening starts from the filters of the view.
	$effect(() => {
		if (open) untrack(() => (draft = toDraft(filters)));
	});

	/** Suggestions of a field from the server as the user types; a late answer is dropped. */
	function suggestions(field: OrderValueField, text: () => string) {
		let items = $state.raw<{ value: string }[]>([]);
		let seq = 0;
		$effect(() => {
			if (!open) return;
			const query = text().trim();
			const timer = setTimeout(() => {
				const mine = ++seq;
				orderValues(field, query).then(
					(values) => {
						if (mine === seq) items = values.map((value) => ({ value }));
					},
					// Without suggestions the field still takes any text.
					() => {}
				);
			}, SUGGEST_MS);
			return () => clearTimeout(timer);
		});
		return {
			get items() {
				return items;
			}
		};
	}
	const types = suggestions('device_type', () => draft.deviceType);
	const brands = suggestions('brand', () => draft.brand);

	// The masters are read when the menu opens: disabled ones stay, an old order may have them.
	let masters = $state.raw<MasterList>(null);
	$effect(() => {
		if (!open) return;
		let current = true;
		listMasters().then(
			(list) => {
				if (current) masters = list;
			},
			// Without them the filter is «Любой» and the chosen master («Мастер № 5»).
			() => {
				if (current && masters === null) masters = 'failed';
			}
		);
		return () => {
			current = false;
		};
	});
	/** «Любой», the masters, and the name of an old saved view as one more choice. */
	const masterOptions = $derived([
		{ value: ANY_MASTER, label: t.anyMaster },
		...filterMasterItems(masters, draft.masterId),
		...(draft.master && draft.masterId === undefined
			? [{ value: BY_NAME, label: draft.master }]
			: [])
	]);

	const pointItems = $derived([
		{ value: TOP_BAR_POINT, label: t.pointFromTopBar },
		...app.points
			.filter((p) => !p.archived || p.id === draft.pointId)
			.map((p) => ({ value: p.id, label: p.name }))
	]);
	const statusItems = $derived(
		[...app.statuses]
			.filter((s) => !s.archived || draft.statusIds.includes(s.id))
			.sort((a, b) => a.sort_order - b.sort_order)
	);
	const count = $derived(Object.keys(filters).length);
	const buttonText = $derived(count > 0 ? `${t.filters} · ${count}` : t.filters);

	function toggleStatus(id: number, on: boolean) {
		const rest = draft.statusIds.filter((x) => x !== id);
		draft.statusIds = on ? [...rest, id] : rest;
	}

	function apply() {
		if (hasInvalid) return;
		onapply(fromDraft(draft));
		open = false;
	}
	function clear() {
		onapply({});
		open = false;
	}

	// «Сохранить вид…» applies what is being edited, like «Применить» (the list shows what is saved),
	// then asks for a name; the view goes to «Мои виды» with those filters.
	const formId = $props.id();
	let saveOpen = $state(false);
	let name = $state('');
	let nameError = $state<string | undefined>();
	let saving: OrderFilters = {};
	async function startSave() {
		if (hasInvalid) return;
		saving = fromDraft(draft);
		onapply(saving);
		open = false;
		await tick();
		name = '';
		nameError = undefined;
		saveOpen = true;
	}
	$effect(() => {
		if (saveOpen) return submitShortcut(save);
	});
	function save() {
		if (name.trim() === '') {
			nameError = t.viewNameRequired;
			return;
		}
		saveOpen = false;
		onsaveview(name, saving);
	}
</script>

<Popover bind:open width="22rem" label={t.filters}>
	{#snippet trigger(props)}
		{#if compact}
			<IconButton {...props} label={buttonText} icon={ListFilter} variant="secondary" />
		{:else}
			<Button {...props} icon={ListFilter} iconEnd={ChevronDown}>{buttonText}</Button>
		{/if}
	{/snippet}
	<form
		class="menu"
		novalidate
		onsubmit={(e) => {
			e.preventDefault();
			apply();
		}}
	>
		<div class="fields">
			<Combobox label={t.deviceType} bind:value={draft.deviceType} items={types.items} />
			<Combobox label={t.brand} bind:value={draft.brand} items={brands.items} />
			<Select
				label={t.master}
				items={masterOptions}
				bind:value={
					() => draft.masterId ?? (draft.master ? BY_NAME : ANY_MASTER),
					(v) => {
						if (v === BY_NAME) return;
						draft.masterId = v === null || v === ANY_MASTER ? undefined : v;
						draft.master = '';
					}
				}
			/>
			{#if pointItems.length > 1}
				<Select
					label={t.point}
					items={pointItems}
					bind:value={() => draft.pointId, (v) => (draft.pointId = v ?? TOP_BAR_POINT)}
				/>
			{/if}
			<fieldset>
				<legend class="label-caps">{t.statuses}</legend>
				<div class="checks">
					{#each statusItems as s (s.id)}
						<Checkbox
							label={s.name}
							bind:checked={() => draft.statusIds.includes(s.id), (on) => toggleStatus(s.id, on)}
						/>
					{/each}
				</div>
			</fieldset>
			<fieldset>
				<legend class="label-caps">{t.received}</legend>
				<div class="period">
					<DateInput
						label={t.from}
						max={draft.received[1] ?? undefined}
						bind:value={draft.received[0]}
						bind:invalid={invalid.rf}
					/>
					<DateInput
						label={t.to}
						min={draft.received[0] ?? undefined}
						bind:value={draft.received[1]}
						bind:invalid={invalid.rt}
					/>
				</div>
			</fieldset>
			<fieldset>
				<legend class="label-caps">{t.issuedAt}</legend>
				<div class="period">
					<DateInput
						label={t.from}
						max={draft.issued[1] ?? undefined}
						bind:value={draft.issued[0]}
						bind:invalid={invalid.if}
					/>
					<DateInput
						label={t.to}
						min={draft.issued[0] ?? undefined}
						bind:value={draft.issued[1]}
						bind:invalid={invalid.it}
					/>
				</div>
			</fieldset>
			<fieldset>
				<legend class="label-caps">{t.marks}</legend>
				<div class="checks">
					<Checkbox
						label={ru.flags.needCall}
						bind:checked={
							() => draft.needCall === true, (on) => (draft.needCall = on ? true : undefined)
						}
					/>
					<Checkbox
						label={ru.flags.needParts}
						bind:checked={
							() => draft.needParts === true, (on) => (draft.needParts = on ? true : undefined)
						}
					/>
				</div>
			</fieldset>
		</div>
		<div class="actions">
			<Button variant="ghost" onclick={() => void startSave()}>{t.saveView}</Button>
			<span class="end">
				<Button onclick={clear}>{t.clear}</Button>
				<Button variant="primary" type="submit" disabled={hasInvalid}>{t.apply}</Button>
			</span>
		</div>
	</form>
</Popover>

<Dialog bind:open={saveOpen} title={t.saveViewTitle} size="sm">
	<form
		id={formId}
		novalidate
		onsubmit={(e) => {
			e.preventDefault();
			save();
		}}
	>
		<TextInput label={t.viewName} bind:value={name} error={nameError} required autofocus />
	</form>
	{#snippet footer()}
		<Button onclick={() => (saveOpen = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" type="submit" form={formId} shortcut="Mod+Enter">{t.save}</Button>
	{/snippet}
</Dialog>

<style>
	/* The fields scroll; the buttons stay at the bottom. */
	.menu {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		max-height: min(40rem, calc(100dvh - 8rem));
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		flex: 1;
		min-height: 0;
		/* Room for the focus rings of the fields. */
		margin: calc(-1 * var(--space-1));
		padding: var(--space-1);
		overflow-y: auto;
	}
	fieldset {
		display: flex;
		flex-direction: column;
		gap: var(--space-1-5);
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
	}
	legend {
		padding: 0;
		margin-bottom: var(--space-1-5);
	}
	.checks {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-1-5) var(--space-3);
	}
	.period {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-2);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding-top: var(--space-2);
		border-top: 1px solid var(--border);
	}
	.end {
		display: flex;
		gap: var(--space-2);
	}
</style>
