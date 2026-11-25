<!-- The top of «Отчёты»: the period, the filters, the grouping and «В Excel»; the transfers' tab keeps the
     period and the point only. Every change goes up as a
     whole new view; the screen writes it to the address. -->
<script lang="ts">
	import FileSpreadsheet from '@lucide/svelte/icons/file-spreadsheet';
	import { untrack } from 'svelte';
	import type { ReportGroupBy } from '$api/ReportGroupBy';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { listMasters } from '#lib/api/masters.js';
	import { orderValues } from '#lib/api/orders.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import PeriodPicker from '#lib/ui/PeriodPicker.svelte';
	import Select from '#lib/ui/Select.svelte';
	import { periodRange, type PeriodKind } from '#lib/ui/period.js';
	import type { ReportView } from './view.js';

	let {
		view,
		today,
		exporting = false,
		onchange,
		onexport
	}: {
		view: ReportView;
		/** The enterprise's local day: «Свой период» starts from the period chosen before it. */
		today: string;
		exporting?: boolean;
		/** A change as a function of the latest view: two quick changes do not undo each other.
		 *  `replace` — typed text rewrites the entry of the history instead of adding one. */
		onchange: (change: (v: ReportView) => ReportView, replace?: boolean) => void;
		onexport: () => void;
	} = $props();

	const t = ru.reports;
	/** A pause after the last key before the typed text becomes a filter. */
	const TYPING_MS = 500;
	/** «Любой» of the selects: masters and points count from 1, the sources are text. */
	const ANY = 0;
	const ANY_SOURCE = '';
	const SUGGEST_MS = 150;

	function changePeriod(kind: PeriodKind, from: string | null, to: string | null) {
		onchange((v) => {
			// «Свой период» opens with the days of the period it replaces; after «Всё время» they stay empty.
			if (kind === 'custom' && v.period !== 'custom' && from === null && to === null) {
				[from, to] = periodRange(v.period, today);
			}
			return {
				...v,
				period: kind,
				from: kind === 'custom' ? from : null,
				to: kind === 'custom' ? to : null
			};
		});
	}

	// The masters (the disabled ones too: old orders have them) and the sources of clients.
	let masters = $state.raw<{ value: number; label: string }[]>([]);
	let sources = $state.raw<{ value: string; label: string }[]>([]);
	$effect(() => {
		let current = true;
		listMasters().then(
			(list) => {
				if (current) masters = list.map((m) => ({ value: m.id, label: m.display_name }));
			},
			// Without them the filter is «Любой».
			() => {}
		);
		listDictionary('client_source').then(
			(list) => {
				if (current) sources = list.map((s) => ({ value: s.value, label: s.value }));
			},
			() => {}
		);
		return () => {
			current = false;
		};
	});
	const masterItems = $derived([{ value: ANY, label: t.anyMaster }, ...masters]);
	const pointItems = $derived([
		{ value: ANY, label: t.anyPoint },
		...app.points.filter((p) => !p.archived).map((p) => ({ value: p.id, label: p.name }))
	]);
	const sourceItems = $derived([
		{ value: ANY_SOURCE, label: t.anySource },
		...sources,
		// A source of the address that the dictionary no longer has stays a choice.
		...(view.source && !sources.some((s) => s.value === view.source)
			? [{ value: view.source, label: view.source }]
			: [])
	]);
	const groupItems: { value: ReportGroupBy; label: string }[] = [
		{ value: 'master', label: t.groupByMaster },
		{ value: 'service_point', label: t.groupByPoint },
		{ value: 'device_type', label: t.groupByType },
		{ value: 'brand', label: t.groupByBrand },
		{ value: 'source', label: t.groupBySource }
	];

	/** What is typed in a text filter: it becomes the view after a pause, at Enter, a choice or a blur. */
	function typed(field: 'deviceType' | 'brand', valueField: 'device_type' | 'brand') {
		let text = $state(view[field] ?? '');
		let items = $state.raw<{ value: string }[]>([]);
		let timer: ReturnType<typeof setTimeout> | undefined;
		let seq = 0;
		// Back or forward in the history changes the filter under the field; what was typed meanwhile and not
		// yet applied stays unless the filter itself changed.
		let seen = view[field];
		$effect(() => {
			const next = view[field];
			if (next === seen) return;
			seen = next;
			untrack(() => (text = next ?? ''));
		});
		// Suggestions of the orders' list: the same source as its filters.
		$effect(() => {
			const query = text.trim();
			const id = setTimeout(() => {
				const mine = ++seq;
				orderValues(valueField, query).then(
					(values) => {
						if (mine === seq) items = values.map((value) => ({ value }));
					},
					() => {}
				);
			}, SUGGEST_MS);
			return () => clearTimeout(id);
		});
		function apply() {
			clearTimeout(timer);
			const value = text.trim() || undefined;
			if (value === view[field]) return;
			onchange((v) => ({ ...v, [field]: value }), true);
		}
		return {
			get text() {
				return text;
			},
			set text(v: string) {
				text = v;
				clearTimeout(timer);
				timer = setTimeout(apply, TYPING_MS);
			},
			get items() {
				return items;
			},
			apply
		};
	}
	const deviceType = typed('deviceType', 'device_type');
	const brand = typed('brand', 'brand');
</script>

<div class="filters">
	<PeriodPicker kind={view.period} from={view.from} to={view.to} onchange={changePeriod} />
	<div class="fields">
		{#if view.tab === 'orders'}
			<Select
				label={t.master}
				items={masterItems}
				bind:value={
					() => view.masterUserId ?? ANY,
					(id) =>
						onchange((v) => ({ ...v, masterUserId: id === null || id === ANY ? undefined : id }))
				}
			/>
		{/if}
		{#if pointItems.length > 1}
			<Select
				label={t.point}
				items={pointItems}
				bind:value={
					() => view.servicePointId ?? ANY,
					(id) =>
						onchange((v) => ({ ...v, servicePointId: id === null || id === ANY ? undefined : id }))
				}
			/>
		{/if}
		{#if view.tab === 'orders'}
			<Combobox
				label={t.deviceType}
				items={deviceType.items}
				bind:value={deviceType.text}
				onselect={deviceType.apply}
				onenter={deviceType.apply}
				onblur={deviceType.apply}
			/>
			<Combobox
				label={t.brand}
				items={brand.items}
				bind:value={brand.text}
				onselect={brand.apply}
				onenter={brand.apply}
				onblur={brand.apply}
			/>
			<Select
				label={t.source}
				items={sourceItems}
				bind:value={
					() => view.source ?? ANY_SOURCE,
					(name) =>
						onchange((v) => ({
							...v,
							source: name === null || name === ANY_SOURCE ? undefined : name
						}))
				}
			/>
			<Select
				label={t.groupBy}
				items={groupItems}
				bind:value={
					() => view.groupBy, (by) => onchange((v) => ({ ...v, groupBy: by ?? 'master' }))
				}
			/>
			<div class="export">
				{#if app.can('data_export')}
					<Button icon={FileSpreadsheet} onclick={onexport} disabled={exporting}>{t.toExcel}</Button
					>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style>
	.filters {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.fields {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--space-3);
	}
	.fields > :global(*) {
		min-width: 10rem;
	}
	.export {
		margin-left: auto;
		min-width: 0;
	}
</style>
