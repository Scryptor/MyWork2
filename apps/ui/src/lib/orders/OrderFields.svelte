<!-- The fields of an order (spec 6.2), shared by the panel and the full card: text is saved a moment after
     the last key or on leaving the field, choices and switches at once. A conflict with another computer
     marks the disputed fields with «Оставить моё» / «Взять новое». -->
<script lang="ts">
	import Eye from '@lucide/svelte/icons/eye';
	import EyeOff from '@lucide/svelte/icons/eye-off';
	import { untrack } from 'svelte';
	import { resolve } from '$app/paths';
	import type { Client } from '$api/Client';
	import type { Order } from '$api/Order';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { clientHref } from '#lib/clients/view.js';
	import {
		countLabel,
		displayName,
		formatMoney,
		formatPhone,
		formatRelative,
		dialNumber
	} from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import Badge from '#lib/ui/Badge.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import FieldDispute from '#lib/ui/FieldDispute.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import MoneyInput from '#lib/ui/MoneyInput.svelte';
	import PatternField from '#lib/ui/PatternField.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import type { Autosave } from './autosave.js';
	import type { Conflict, OrderField, OrderValues } from './editor.svelte.js';
	import { masterChoices, masterName, masterOf, masterValue, type MasterList } from './masters.js';
	import { orderCost } from './parts.svelte.js';
	import { patternText } from './pattern.js';
	import { estimateText, isNoWarranty, toPay } from './view.js';

	let {
		order,
		autosave,
		client,
		dictionaries,
		readonly = false,
		editDevice = false,
		idPrefix = 'order',
		partsTotal = 0,
		conflicts = new Map(),
		flashed = new Set(),
		onkeep,
		ontake,
		onflashend,
		onopenclient
	}: {
		order: Order;
		autosave: Autosave<OrderValues>;
		client: Client | null;
		dictionaries: {
			/** The masters: users marked «Мастер»; `null` until read, `'failed'` when not read. */
			masters: MasterList;
			warranty: string[];
			device_type?: string[];
			brand?: string[];
			color?: string[];
		};
		/** No editing (no right to edit orders). An issued order keeps its fields editable. */
		readonly?: boolean;
		/** The device as fields (the full card), not as a line of text (the panel). */
		editDevice?: boolean;
		/** Field ids are `<idPrefix>-<field>`: the same field gets focus back on another order. */
		idPrefix?: string;
		/** The parts written off from the stock into the order, in kopecks: the cost line under «Расходы». */
		partsTotal?: number;
		conflicts?: ReadonlyMap<OrderField, Conflict>;
		flashed?: ReadonlySet<OrderField>;
		onkeep?: (field: OrderField) => void;
		ontake?: (field: OrderField) => void;
		onflashend?: (field: OrderField) => void;
		/** «открыть» at the client: the panel closes before «Клиенты» opens; without it a plain link. */
		onopenclient?: (e: MouseEvent) => void;
	} = $props();

	// «открыть»: the client's panel on «Клиенты», for those who may see the list.
	const canOpenClient = $derived(app.can('clients_view') || app.can('orders_create'));

	const t = ru.order;
	const LABELS: Partial<Record<OrderField, string>> = {
		device_type: t.deviceType,
		brand: t.brand,
		model: t.model,
		color: t.color,
		serial: t.serial,
		device_password: t.devicePassword,
		device_pattern: t.devicePattern,
		completeness: t.completeness,
		device_condition: t.condition,
		malfunction: t.malfunction,
		work_done: t.workDone,
		master_user_id: t.master,
		service_point_id: t.point,
		final_price: t.finalPrice,
		prepayment: t.prepayment,
		discount: t.discount,
		extra_costs: t.extraCosts,
		warranty: t.warrantyTerm,
		need_call: ru.flags.needCall,
		need_parts: ru.flags.needParts,
		internal_comment: t.comment
	};
	/** The device's fields, in their order on screen. */
	const DEVICE = [
		'device_type',
		'brand',
		'model',
		'color',
		'serial',
		'device_password',
		'device_pattern'
	] as const satisfies readonly OrderField[];
	/** What came with the device and how it looks: text of any length. */
	const DEVICE_NOTES = [
		'completeness',
		'device_condition'
	] as const satisfies readonly OrderField[];
	const DEVICE_ALL = [...DEVICE, ...DEVICE_NOTES];
	const idOf = (f: OrderField) => `${idPrefix}-${f}`;
	/** «Что это?» of the fields whose meaning the label does not tell. */
	const HELP: Partial<Record<OrderField, readonly string[]>> = {
		final_price: ru.help.order.finalPrice,
		prepayment: ru.help.order.prepayment,
		extra_costs: ru.help.order.extraCosts,
		warranty: ru.help.order.warranty,
		need_call: ru.help.order.needCall,
		need_parts: ru.help.order.needParts
	};

	/** Device fields chosen from a dictionary; the others are typed. */
	type DeviceChoice = 'device_type' | 'brand' | 'color';
	const DEVICE_CHOICES: DeviceChoice[] = ['device_type', 'brand', 'color'];
	const isChoice = (f: OrderField): f is DeviceChoice => DEVICE_CHOICES.includes(f as DeviceChoice);
	const choiceItems = $derived(
		Object.fromEntries(
			DEVICE_CHOICES.map((f) => [f, (dictionaries[f] ?? []).map((value) => ({ value }))])
		) as Record<DeviceChoice, { value: string }[]>
	);
	const masterOptions = $derived(
		masterChoices(dictionaries.masters, order.master_user_id, order.master)
	);
	const warrantyItems = $derived(dictionaries.warranty.map((value) => ({ value })));

	/** «Без точки» in the select of points: selects take numbers, an order takes `null` (ids start at 1). */
	const NO_POINT = 0;
	const pointLabel = (id: number) => app.points.find((p) => p.id === id)?.name ?? t.pointNumber(id);
	/** «Без точки» and the points not in the archive; the order's own archived point stays, not choosable. */
	const pointOptions = $derived.by(() => {
		const items: { value: number; label: string; disabled?: boolean }[] = [
			{ value: NO_POINT, label: t.noPoint },
			...app.activePoints.map((p) => ({ value: p.id, label: p.name }))
		];
		const current = order.service_point_id;
		if (current !== null && !items.some((i) => i.value === current)) {
			const known = app.points.find((p) => p.id === current);
			const label = known ? t.pointArchived(known.name) : t.pointNumber(current);
			items.push({ value: current, label, disabled: true });
		}
		return items;
	});

	/** What the field shows: own unsaved value, or the order's. */
	const shown = <K extends OrderField>(f: K): OrderValues[K] => autosave.value(f, order[f]);

	// Money: an amount goes as it is typed; while the text is not an amount nothing is sent and the field
	// keeps the last amount it wrote, so leaving it shows the error instead of the old sum.
	type MoneyField = 'final_price' | 'prepayment' | 'discount' | 'extra_costs';
	const MONEY: MoneyField[] = ['final_price', 'prepayment', 'discount', 'extra_costs'];
	let invalid = $state<Record<MoneyField, boolean>>({
		final_price: false,
		prepayment: false,
		discount: false,
		extra_costs: false
	});
	const held: Partial<Record<MoneyField, number | null>> = {};
	function money(f: MoneyField): number | null {
		return invalid[f] && f in held ? (held[f] ?? null) : shown(f);
	}
	function setMoney(f: MoneyField, v: number | null) {
		held[f] = v;
		if (f === 'final_price') autosave.edit(f, v);
		else autosave.edit(f, v ?? 0);
	}
	$effect(() => {
		for (const f of MONEY) if (invalid[f]) untrack(() => autosave.discard(f));
	});

	const pay = $derived(
		MONEY.some((f) => invalid[f])
			? null
			: toPay({
					final_price: money('final_price'),
					prepayment: money('prepayment') ?? 0,
					discount: money('discount') ?? 0
				})
	);
	const estimate = $derived(estimateText(order));

	/** The password and the pattern key are shown (one button for both). */
	let revealed = $state(false);
	const revealLabel = $derived.by(() => {
		if (order.device_password && order.device_pattern)
			return revealed ? t.hideSecrets : t.showSecrets;
		if (order.device_pattern) return revealed ? t.hidePattern : t.showPattern;
		return revealed ? ru.fields.hidePassword : ru.fields.showPassword;
	});
	let allPhones = $state(false);
	const phones = $derived(client?.phones ?? []);
	const mainPhone = $derived(phones[0]);
	const otherPhones = $derived(phones.slice(1));
	const deviceLine = $derived(
		[order.device_type, order.brand, order.model, order.color].filter(Boolean).join(' · ')
	);

	/**
	 * Leaving a field saves what was typed in it; then it shows the text as the server keeps it. Another
	 * window taking focus (Alt-Tab) is not leaving: the field gets focus back with the text as typed.
	 */
	function leave(e: FocusEvent) {
		void autosave.flush().catch(() => {});
		if (e.relatedTarget === null || !document.hasFocus()) return;
		autosave.release();
	}

	const flash = (fields: readonly OrderField[]) => fields.some((f) => flashed.has(f));
	function endFlash(fields: readonly OrderField[]) {
		for (const f of fields) onflashend?.(f);
	}

	function asText(f: OrderField, v: unknown): string {
		if (f === 'service_point_id') return typeof v === 'number' ? pointLabel(v) : t.noPoint;
		if (typeof v === 'boolean') return v ? t.yes : t.no;
		if (v === null || v === '') return t.empty;
		if (f === 'master_user_id' && typeof v === 'number') {
			return masterName(dictionaries.masters, v, String(v));
		}
		if (MONEY.includes(f as MoneyField) && typeof v === 'number') return formatMoney(v);
		if (f === 'device_pattern' && typeof v === 'string') return patternText(v);
		return String(v);
	}
</script>

{#snippet dispute(f: OrderField)}
	{@const c = conflicts.get(f)}
	{#if c}
		<FieldDispute
			label={LABELS[f] ?? f}
			theirs={asText(f, c.theirs)}
			onkeep={() => onkeep?.(f)}
			ontake={() => ontake?.(f)}
		/>
	{/if}
{/snippet}

<div class="fields" onfocusout={leave}>
	{#if conflicts.size > 0}
		<Callout tone="warning" title={t.conflictTitle}>{t.conflictText}</Callout>
	{/if}

	<Section title={t.client}>
		{#snippet actions()}
			{#if canOpenClient}
				<a class="open-client" href={resolve(clientHref(order.client_id))} onclick={onopenclient}
					>{ru.client.open}</a
				>
			{/if}
		{/snippet}
		{#if client}
			<p class="line">
				<span class="strong">{displayName(client.full_name)}</span>
				{#if client.blacklisted}<Badge tone="danger">{ru.flags.blacklisted}</Badge>{/if}
			</p>
			{#if mainPhone}
				<p class="line num">
					<a href="tel:{dialNumber(mainPhone.phone)}">{formatPhone(mainPhone.phone)}</a>
					{#if mainPhone.note}<span class="muted">{mainPhone.note}</span>{/if}
					{#if otherPhones.length > 0 && !allPhones}
						<button type="button" class="link" onclick={() => (allPhones = true)}>
							{t.morePhones}
							{countLabel(otherPhones.length, t.phoneForms)}
						</button>
					{/if}
				</p>
			{/if}
			{#if allPhones}
				{#each otherPhones as p, i (i)}
					<p class="line num">
						<a href="tel:{dialNumber(p.phone)}">{formatPhone(p.phone)}</a>
						{#if p.note}<span class="muted">{p.note}</span>{/if}
					</p>
				{/each}
			{/if}
		{:else}
			<p class="line muted">{t.clientNumber(order.client_id)}</p>
		{/if}
	</Section>

	<Section title={t.work}>
		<div
			class="flashable"
			class:flash={flash(['malfunction'])}
			onanimationend={() => endFlash(['malfunction'])}
		>
			<Textarea
				id={idOf('malfunction')}
				label={t.malfunction}
				disabled={readonly}
				bind:value={() => shown('malfunction'), (v) => autosave.edit('malfunction', v)}
			/>
			{@render dispute('malfunction')}
		</div>
		<div class="pair">
			<div
				class="flashable"
				class:flash={flash(['work_done'])}
				onanimationend={() => endFlash(['work_done'])}
			>
				<Textarea
					id={idOf('work_done')}
					label={t.workDone}
					disabled={readonly}
					bind:value={() => shown('work_done'), (v) => autosave.edit('work_done', v)}
				/>
				{@render dispute('work_done')}
			</div>
			<div class="side">
				<div
					class="flashable"
					class:flash={flash(['master_user_id'])}
					onanimationend={() => endFlash(['master_user_id'])}
				>
					<Select
						id={idOf('master_user_id')}
						label={t.master}
						items={masterOptions}
						disabled={readonly}
						bind:value={
							() => masterValue(shown('master_user_id')),
							(v) => autosave.commit('master_user_id', masterOf(v))
						}
					/>
					{@render dispute('master_user_id')}
				</div>
				{#if app.points.length > 0}
					<div
						class="flashable"
						class:flash={flash(['service_point_id'])}
						onanimationend={() => endFlash(['service_point_id'])}
					>
						<Select
							id={idOf('service_point_id')}
							label={t.point}
							items={pointOptions}
							disabled={readonly}
							bind:value={
								() => shown('service_point_id') ?? NO_POINT,
								(v) => autosave.commit('service_point_id', v === null || v === NO_POINT ? null : v)
							}
						/>
						{@render dispute('service_point_id')}
					</div>
				{/if}
			</div>
		</div>
	</Section>

	<Section title={t.money}>
		<p class="line muted num">
			{estimate ? `${t.estimate} ${estimate}` : t.noEstimate}
		</p>
		<div class="money">
			{#each MONEY as f (f)}
				<div class="flashable" class:flash={flash([f])} onanimationend={() => endFlash([f])}>
					<MoneyInput
						id={idOf(f)}
						label={LABELS[f]}
						help={HELP[f]}
						nullable={f === 'final_price'}
						disabled={readonly}
						bind:invalid={invalid[f]}
						bind:value={() => money(f), (v) => setMoney(f, v)}
					/>
					{@render dispute(f)}
					{#if f === 'prepayment' && (money(f) ?? 0) > 0 && order.prepaid_at && !invalid[f]}
						<p class="cost num">
							{t.prepaidAt(formatRelative(order.prepaid_at, serverClock.now()))}
						</p>
					{/if}
					{#if f === 'extra_costs' && partsTotal > 0 && !invalid[f]}
						{@const extra = money(f) ?? 0}
						<p class="cost num">
							{t.cost(
								formatMoney(orderCost(extra, partsTotal)),
								formatMoney(extra),
								formatMoney(partsTotal)
							)}
						</p>
					{/if}
				</div>
			{/each}
		</div>
		<p class="to-pay num" class:negative={pay !== null && pay < 0}>
			{t.toPay} <strong>{pay === null ? '—' : formatMoney(pay)}</strong>
		</p>
		{#if pay !== null && pay < 0}<p class="error">{t.toPayNegative}</p>{/if}
	</Section>

	{#if editDevice}
		<Section title={t.device}>
			<div class="device">
				{#each DEVICE as f (f)}
					<div class="flashable" class:flash={flash([f])} onanimationend={() => endFlash([f])}>
						{#if isChoice(f)}
							<Combobox
								id={idOf(f)}
								label={LABELS[f]}
								items={choiceItems[f]}
								disabled={readonly}
								bind:value={() => shown(f), (v) => autosave.edit(f, v)}
								onselect={(v) => autosave.commit(f, v)}
							/>
						{:else if f === 'device_pattern'}
							<PatternField
								id={idOf(f)}
								label={t.devicePattern}
								disabled={readonly}
								value={shown(f)}
								onchange={(v) => autosave.commit(f, v)}
							/>
						{:else}
							<TextInput
								id={idOf(f)}
								label={LABELS[f]}
								type={f === 'device_password' ? 'password' : 'text'}
								revealable={f === 'device_password'}
								autocomplete="off"
								disabled={readonly}
								bind:value={() => shown(f), (v) => autosave.edit(f, v)}
							/>
						{/if}
						{@render dispute(f)}
					</div>
				{/each}
			</div>
			<div class="notes">
				{#each DEVICE_NOTES as f (f)}
					<div class="flashable" class:flash={flash([f])} onanimationend={() => endFlash([f])}>
						<Textarea
							id={idOf(f)}
							label={LABELS[f]}
							minRows={1}
							disabled={readonly}
							bind:value={() => shown(f), (v) => autosave.edit(f, v)}
						/>
						{@render dispute(f)}
					</div>
				{/each}
			</div>
		</Section>
	{:else}
		<div
			class="flashable"
			class:flash={flash(DEVICE_ALL)}
			onanimationend={() => endFlash(DEVICE_ALL)}
		>
			<Section title={t.device}>
				{#if deviceLine}<p class="line">{deviceLine}</p>{/if}
				{#if order.serial || order.device_password || order.device_pattern}
					<p class="line muted num">
						{#if order.serial}<span>{t.serial} {order.serial}</span>{/if}
						{#if order.device_password || order.device_pattern}
							<span class="secrets">
								{#if order.device_password}
									<span>{t.password} {revealed ? order.device_password : '••••'}</span>
								{/if}
								{#if order.device_pattern}
									<span>
										{t.pattern}
										{revealed ? patternText(order.device_pattern) : '••••'}
									</span>
								{/if}
								<button
									type="button"
									class="reveal"
									aria-pressed={revealed}
									aria-label={revealLabel}
									onclick={() => (revealed = !revealed)}
								>
									{#if revealed}
										<EyeOff size={16} aria-hidden="true" />
									{:else}
										<Eye size={16} aria-hidden="true" />
									{/if}
								</button>
							</span>
						{/if}
					</p>
				{/if}
				{#each DEVICE_NOTES as f (f)}
					{#if order[f]}<p class="line muted">{LABELS[f]}: {order[f]}</p>{/if}
				{/each}
			</Section>
		</div>
	{/if}

	<Section title={t.warranty}>
		<div
			class="flashable"
			class:flash={flash(['warranty'])}
			onanimationend={() => endFlash(['warranty'])}
		>
			<Combobox
				id={idOf('warranty')}
				label={t.warrantyTerm}
				help={HELP.warranty}
				items={warrantyItems}
				disabled={readonly}
				hint={isNoWarranty(order) && shown('warranty') === order.warranty
					? t.noWarranty
					: undefined}
				bind:value={() => shown('warranty'), (v) => autosave.edit('warranty', v)}
				onselect={(v) => autosave.commit('warranty', v)}
			/>
			{@render dispute('warranty')}
		</div>
	</Section>

	<Section title={t.marks}>
		<div class="marks">
			{#each ['need_call', 'need_parts'] as const as f (f)}
				<div class="flashable" class:flash={flash([f])} onanimationend={() => endFlash([f])}>
					<Switch
						label={LABELS[f] ?? f}
						help={HELP[f]}
						disabled={readonly}
						bind:checked={() => shown(f), (v) => autosave.commit(f, v)}
					/>
					{@render dispute(f)}
				</div>
			{/each}
		</div>
	</Section>

	<div
		class="flashable"
		class:flash={flash(['internal_comment'])}
		onanimationend={() => endFlash(['internal_comment'])}
	>
		<Textarea
			id={idOf('internal_comment')}
			label={t.comment}
			disabled={readonly}
			bind:value={() => shown('internal_comment'), (v) => autosave.edit('internal_comment', v)}
		/>
		{@render dispute('internal_comment')}
	</div>
</div>

<style>
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin: 0;
	}
	.strong {
		font-weight: 500;
	}
	.muted {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	a,
	.link {
		color: var(--accent);
	}
	.open-client {
		font-size: var(--text-sm);
	}
	.link {
		padding: 0;
		border: 0;
		background: transparent;
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}
	.link:hover {
		text-decoration: underline;
	}
	.secrets {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.reveal {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		vertical-align: middle;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--text-3);
		cursor: pointer;
	}
	.reveal:hover {
		color: var(--text);
	}
	.pair {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 11rem);
		align-items: start;
		gap: var(--space-3);
	}
	.side {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
	}
	.device {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		align-items: start;
		gap: var(--space-3);
	}
	.notes {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		align-items: start;
		gap: var(--space-3);
	}
	/* Four fields two by two: none alone on a row («Расходы» and its cost line too). */
	.money {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		align-items: start;
		gap: var(--space-3);
	}
	.marks {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-4);
	}
	.cost {
		margin: var(--space-1) 0 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.to-pay {
		margin: 0;
		font-size: var(--text-lg);
	}
	.to-pay strong {
		font-weight: 600;
	}
	.to-pay.negative {
		color: var(--danger-text);
	}
	.error {
		margin: 0;
		color: var(--danger-text);
		font-size: var(--text-sm);
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
	@media (pointer: coarse) {
		.reveal,
		.link {
			min-width: var(--control-md);
			min-height: var(--control-md);
		}
	}
	:global([data-width='xs']) .pair,
	:global([data-width='xs']) .device,
	:global([data-width='xs']) .notes,
	:global([data-width='xs']) .money {
		grid-template-columns: minmax(0, 1fr);
	}
</style>
