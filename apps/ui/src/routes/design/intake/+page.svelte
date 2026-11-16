<!-- Demo of the intake (spec 6.5): a wide panel with the form and a summary on the right; the draft
     stays on this device until the order is accepted. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { tick, untrack } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { DEMO_NOW, DEMO_STATUSES, makeDictionaries, makeOrders } from '#lib/demo/fake.js';
	import {
		emptyDraft,
		findClient,
		findSerial,
		INTAKE_DRAFT_KEY,
		missingRequired,
		parseDraft,
		REQUIRED_LABELS,
		type IntakeDraft,
		type RequiredKey
	} from '#lib/demo/intake.js';
	import { daysSince, deviceOf, type DemoPhoto } from '#lib/demo/orders.js';
	import { countLabel, displayName, formatDate, formatPhone } from '#lib/format/index.js';
	import {
		Avatar,
		Badge,
		Button,
		Callout,
		Card,
		ChipGroup,
		Combobox,
		FileDrop,
		MoneyInput,
		PhoneInput,
		PhotoStrip,
		Section,
		Select,
		Sheet,
		StatusBadge,
		Switch,
		Textarea,
		TextInput,
		toast
	} from '#lib/ui/index.js';

	const DICTIONARIES = makeDictionaries();
	const ORDERS = makeOrders(2000);
	const ORDERS_FORMS = ['заказ', 'заказа', 'заказов'] as const;
	const MAX_PHOTOS = 10;
	/** The switches «Акт приёма» and «Наклейка» are remembered on this device. */
	const PRINT_KEY = 'mw.demo.intakePrint';
	const FIELD_IDS: Record<RequiredKey, string> = {
		contact: 'intake-phone',
		deviceType: 'intake-type',
		malfunction: 'intake-malfunction'
	};
	type MoneyKey = 'estimateFrom' | 'estimateTo' | 'prepayment';
	/** The money fields, in the order of the form. */
	const MONEY_FIELDS: Record<MoneyKey, { label: string; id: string }> = {
		estimateFrom: { label: 'Оценка от', id: 'intake-estimate-from' },
		estimateTo: { label: 'Оценка до', id: 'intake-estimate-to' },
		prepayment: { label: 'Предоплата', id: 'intake-prepayment' }
	};
	const MONEY_KEYS = Object.keys(MONEY_FIELDS) as MoneyKey[];

	const statusById = new Map(DEMO_STATUSES.map((s) => [s.id, s]));
	// A new order starts in work: the first status of kind «Диагностика» unless chosen otherwise.
	const startStatuses = DEMO_STATUSES.filter(
		(s) => !s.archived && (s.kind === 'diagnostics' || s.kind === 'open')
	);
	const defaultStatus = startStatuses.find((s) => s.kind === 'diagnostics')?.id ?? null;
	const statusItems = startStatuses.map((s) => ({ value: s.id, label: s.name }));
	const masterItems = DICTIONARIES.master.map((m) => ({ value: m, label: displayName(m) }));
	const warrantyItems = DICTIONARIES.warranty.map((w) => ({ value: w, label: w }));
	const typeItems = DICTIONARIES.device_type.map((value) => ({ value }));
	const brandItems = DICTIONARIES.brand.map((value) => ({ value }));
	const sourceItems = DICTIONARIES.source.map((value) => ({ value }));
	const frequent = DICTIONARIES.malfunction.slice(0, 8);

	let open = $state(false);
	let draft = $state<IntakeDraft>(readDraft());
	let attempted = $state(false);
	let photos = $state.raw<DemoPhoto[]>([]);
	let printing = $state(readPrinting());
	let accepted = $state.raw<number[]>([]);
	/** Money fields holding text that is not an amount: the order is not accepted until they are fixed. */
	let wrongMoney = $state<Record<MoneyKey, boolean>>({
		estimateFrom: false,
		estimateTo: false,
		prepayment: false
	});
	let nextNumber = Math.max(...ORDERS.map((o) => o.id)) + 1;
	let photoSeq = 0;
	let drop = $state<ReturnType<typeof FileDrop>>();

	// The panel opens once the navigation has settled. After a navigation SvelteKit focuses <body> and,
	// on a timer, clears the selection if it looks unchanged; in WebKit that also takes the caret out of
	// «Телефон» focused in between, and typing goes nowhere. This timer runs after SvelteKit's.
	afterNavigate(() => {
		setTimeout(() => (open = true));
	});

	function readDraft(): IntakeDraft {
		try {
			return parseDraft(localStorage.getItem(INTAKE_DRAFT_KEY));
		} catch {
			return emptyDraft();
		}
	}
	// Every change of the form is kept on the device at once.
	$effect(() => {
		const json = JSON.stringify(draft);
		untrack(() => {
			try {
				localStorage.setItem(INTAKE_DRAFT_KEY, json);
			} catch {
				// storage unavailable: the draft lives as long as the page
			}
		});
	});

	function readPrinting(): { act: boolean; label: boolean } {
		try {
			const saved: unknown = JSON.parse(localStorage.getItem(PRINT_KEY) ?? 'null');
			if (typeof saved === 'object' && saved !== null) {
				const { act, label } = saved as Record<string, unknown>;
				return { act: act !== false, label: label !== false };
			}
		} catch {
			// garbage or no storage: the defaults
		}
		return { act: true, label: true };
	}
	function setPrinting(patch: Partial<{ act: boolean; label: boolean }>) {
		printing = { ...printing, ...patch };
		try {
			localStorage.setItem(PRINT_KEY, JSON.stringify(printing));
		} catch {
			// storage unavailable: the switches work for this page
		}
	}

	const client = $derived(findClient(draft.phone, ORDERS));
	const seen = $derived(findSerial(draft.serial, ORDERS));
	const missing = $derived(missingRequired(draft));
	const wrong = $derived(MONEY_KEYS.filter((k) => wrongMoney[k]));
	const errors = $derived({
		contact: attempted && missing.includes('contact') ? 'Укажите телефон или ФИО' : undefined,
		deviceType: attempted && missing.includes('deviceType') ? 'Укажите тип устройства' : undefined,
		malfunction: attempted && missing.includes('malfunction') ? 'Опишите неисправность' : undefined
	});
	const models = $derived(
		[...new Set(ORDERS.filter((o) => o.brand === draft.brand).map((o) => o.model))]
			.filter((m) => m !== '')
			.sort()
			.map((value) => ({ value }))
	);
	const sourceOf = (id: number) => DICTIONARIES.source[id % DICTIONARIES.source.length] ?? '';

	/** «Этот IMEI уже был» with the warranty check, 30 days from the issue. */
	const seenWarranty = $derived.by(() => {
		if (!seen) return '';
		if (seen.issued_at === null) return 'ещё не выдан';
		const days = daysSince(seen.issued_at, DEMO_NOW);
		return days !== null && days <= 30
			? `выдан ${formatDate(seen.issued_at)}, на гарантии`
			: `выдан ${formatDate(seen.issued_at)}, гарантия закончилась`;
	});

	function addPhotos(files: File[]) {
		const added = files.slice(0, MAX_PHOTOS - photos.length).map((f) => ({
			id: `photo-${++photoSeq}`,
			src: URL.createObjectURL(f),
			alt: f.name
		}));
		photos = [...photos, ...added];
	}
	function removePhoto(id: string) {
		const photo = photos.find((p) => p.id === id);
		if (photo) URL.revokeObjectURL(photo.src);
		photos = photos.filter((p) => p.id !== id);
	}
	$effect(() => () => {
		for (const p of photos) URL.revokeObjectURL(p.src);
	});

	async function accept() {
		if (missing.length > 0 || wrong.length > 0) {
			attempted = true;
			// The first field to fix, in the order of the form: the required ones come before the money.
			const first = missing[0];
			const wrongFirst = wrong[0];
			const id = first ? FIELD_IDS[first] : wrongFirst ? MONEY_FIELDS[wrongFirst].id : undefined;
			if (id) document.getElementById(id)?.focus();
			return;
		}
		const number = nextNumber++;
		open = false;
		accepted = [number, ...accepted];
		draft = emptyDraft();
		attempted = false;
		for (const p of photos) URL.revokeObjectURL(p.src);
		photos = [];
		// Toasts cannot be used under a modal: this one appears once the panel has gone.
		await tick();
		toast.show({ message: `Заказ № ${number} принят`, tone: 'success' });
	}

	$effect(() => registerHotkey('N', () => (open = true)));
	$effect(() => {
		if (!open) return;
		return registerHotkey('Mod+Enter', () => void accept(), {
			allowInFields: true,
			allowWithOverlay: true
		});
	});
</script>

<main class="page">
	<header class="head">
		<h1>Приём устройства</h1>
		<nav class="links" aria-label="Демо">
			<a href={resolve('/design/orders')}>Демо: главный экран</a>
			<a href={resolve('/design')}>Дизайн-система</a>
		</nav>
	</header>
	<p class="muted">
		Широкая панель с затемнением: форма слева, сводка справа. Введённое хранится на этом устройстве,
		пока заказ не принят. Клиент 8 000 000-00-01 есть в вымышленной базе.
	</p>
	<Button variant="primary" icon={Plus} shortcut="N" onclick={() => (open = true)}
		>Новый заказ</Button
	>
	{#if accepted.length > 0}
		<p>Принято в этом демо: {accepted.map((n) => `№ ${n}`).join(', ')}</p>
	{/if}
</main>

<Sheet bind:open title="Новый заказ">
	<div class="layout">
		<div class="form">
			<Section title="Клиент">
				<div class="grid two">
					<PhoneInput
						id={FIELD_IDS.contact}
						label="Телефон"
						bind:value={draft.phone}
						error={errors.contact}
					/>
					{#if !client}
						<TextInput label="ФИО" bind:value={draft.name} error={errors.contact} />
					{/if}
				</div>
				{#if client}
					<section class="client" aria-label="Найденный клиент">
						<div class="client-head">
							<Avatar name={client.name} />
							<strong>{displayName(client.name)}</strong>
							{#if client.blacklisted}<Badge tone="danger">Чёрный список</Badge>{/if}
						</div>
						<p class="num">{formatPhone(client.phone)} · основной</p>
						<p class="muted">
							{countLabel(client.orders.length, ORDERS_FORMS)} · откуда узнали: {sourceOf(
								client.id
							)}
						</p>
					</section>
				{:else}
					<div class="grid two">
						<Combobox label="Откуда узнали" items={sourceItems} bind:value={draft.source} />
					</div>
				{/if}
			</Section>

			<Section title="Устройство">
				<div class="grid three">
					<Combobox
						id={FIELD_IDS.deviceType}
						label="Тип устройства"
						required
						items={typeItems}
						bind:value={draft.deviceType}
						error={errors.deviceType}
					/>
					<Combobox label="Бренд" items={brandItems} bind:value={draft.brand} />
					<Combobox label="Модель" items={models} bind:value={draft.model} />
					<TextInput label="Цвет" bind:value={draft.color} />
					<TextInput label="IMEI или серийный номер" bind:value={draft.serial} />
					<TextInput label="Пароль" type="password" revealable bind:value={draft.password} />
				</div>
				<ChipGroup
					label="Комплектность"
					options={DICTIONARIES.completeness}
					allowCustom
					bind:selected={draft.completeness}
				/>
				<ChipGroup
					label="Внешнее состояние"
					options={DICTIONARIES.condition}
					allowCustom
					bind:selected={draft.condition}
				/>
			</Section>

			<Section title="Неисправность">
				<Textarea
					id={FIELD_IDS.malfunction}
					label="Неисправность"
					required
					bind:value={draft.malfunction}
					error={errors.malfunction}
				/>
				<ChipGroup
					label="Частые неисправности"
					options={frequent}
					bind:selected={draft.malfunctionChips}
				/>
			</Section>

			<FileDrop bind:this={drop} onfiles={addPhotos}>
				<PhotoStrip {photos} onadd={() => drop?.open()} onremove={removePhoto} />
			</FileDrop>

			<Section title="Работа и деньги">
				<div class="grid three">
					<Select label="Мастер" items={masterItems} bind:value={draft.master} />
					<Select
						label="Статус"
						items={statusItems}
						bind:value={() => draft.statusId ?? defaultStatus, (v) => (draft.statusId = v)}
					/>
					<Select
						label="Гарантия"
						items={warrantyItems}
						bind:value={() => draft.warranty, (v) => (draft.warranty = v ?? '')}
					/>
					{#each MONEY_KEYS as key (key)}
						<MoneyInput
							id={MONEY_FIELDS[key].id}
							label={MONEY_FIELDS[key].label}
							bind:value={draft[key]}
							bind:invalid={wrongMoney[key]}
						/>
					{/each}
				</div>
				<Textarea label="Комментарий (только для сервиса)" bind:value={draft.comment} />
			</Section>
		</div>

		<aside class="aside" aria-label="Сводка">
			<Card padding="sm">
				<Section title="Сводка">
					{#if missing.length > 0}
						<p class="missing">Не заполнено: {missing.map((k) => REQUIRED_LABELS[k]).join(', ')}</p>
					{/if}
					{#if wrong.length > 0}
						<p class="missing">
							Неверная сумма: {wrong.map((k) => MONEY_FIELDS[k].label).join(', ')}
						</p>
					{/if}
					{#if missing.length === 0 && wrong.length === 0}
						<p class="ready">Всё обязательное заполнено</p>
					{/if}
				</Section>
			</Card>
			{#if seen}
				<Callout tone="warning" title="Этот IMEI уже был">
					№ {seen.id} · {deviceOf(seen)} · принят {formatDate(seen.received_at)}, {seenWarranty}
				</Callout>
			{/if}
			{#if client}
				<Card padding="sm">
					<Section title="Прошлые заказы">
						<ul class="past">
							{#each client.orders.slice(0, 5) as o (o.id)}
								{@const s = statusById.get(o.status_id)}
								<li>
									<span class="num">№ {o.id} · {formatDate(o.received_at)}</span>
									<span>{deviceOf(o)}</span>
									{#if s}<StatusBadge name={s.name} color={s.color} size="sm" />{/if}
								</li>
							{/each}
						</ul>
					</Section>
				</Card>
			{/if}
		</aside>
	</div>

	{#snippet footer()}
		<div class="print">
			<Switch
				label="Акт приёма"
				bind:checked={() => printing.act, (v) => setPrinting({ act: v })}
			/>
			<Switch
				label="Наклейка"
				bind:checked={() => printing.label, (v) => setPrinting({ label: v })}
			/>
		</div>
		<Button onclick={() => (open = false)}>Отмена</Button>
		<Button variant="primary" shortcut="Mod+Enter" onclick={() => void accept()}>Принять</Button>
	{/snippet}
</Sheet>

<style>
	.page {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-3);
		max-width: 48rem;
		padding: var(--space-6) var(--space-4);
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-4);
	}
	h1 {
		margin: 0;
		font-size: var(--text-xl);
	}
	.links {
		display: flex;
		gap: var(--space-3);
	}
	.links a {
		color: var(--accent);
	}
	.page p {
		margin: 0;
	}
	.muted {
		color: var(--text-3);
	}
	.layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(14rem, 18rem);
		align-items: start;
		gap: var(--space-6);
	}
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		min-width: 0;
	}
	.grid {
		display: grid;
		gap: var(--space-3);
	}
	.grid.two {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}
	.grid.three {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}
	.client {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}
	.client p {
		margin: 0;
	}
	.client-head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.aside {
		position: sticky;
		top: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.missing {
		margin: 0;
		color: var(--warning-text);
	}
	.ready {
		margin: 0;
		color: var(--success-text);
	}
	.past {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.past li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		font-size: var(--text-sm);
	}
	.print {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
		margin-right: auto;
	}
	@media (max-width: 899px) {
		.layout {
			grid-template-columns: minmax(0, 1fr);
		}
		.aside {
			position: static;
		}
		.grid.three {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
	@media (max-width: 599px) {
		.grid.two,
		.grid.three {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
