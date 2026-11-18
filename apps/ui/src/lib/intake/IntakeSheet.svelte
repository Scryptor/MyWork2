<!-- The intake (spec 6.5): a wide sheet with the form and a summary on the right. What is typed waits on
     this device, and the photos on the server under the key of the draft, until the order is accepted or
     the intake is cancelled. -->
<script lang="ts">
	import { tick, untrack } from 'svelte';
	import type { Client } from '$api/Client';
	import type { DictionaryKind } from '$api/DictionaryKind';
	import type { Order } from '$api/Order';
	import type { OrderValueField } from '$api/OrderValueField';
	import { discardDraft } from '#lib/api/attachments.js';
	import { addDictionaryItem, listDictionary } from '#lib/api/catalogs.js';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { createOrder, orderValues } from '#lib/api/orders.js';
	import { app } from '#lib/app/context.svelte.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { readStorage, removeStorage, writeStorage } from '#lib/design/storage.js';
	import { ru } from '#lib/i18n/ru.js';
	import { emitOrderChanged } from '#lib/orders/events.js';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import { createDraftMedia, type Media } from '#lib/orders/photos.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import ChipGroup from '#lib/ui/ChipGroup.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import MoneyInput from '#lib/ui/MoneyInput.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Sheet from '#lib/ui/Sheet.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import ClientStep, { PHONE_ID } from './ClientStep.svelte';
	import {
		acceptedAnswer,
		attemptOf,
		emptyDraft,
		INTAKE_DRAFT_KEY,
		intakeStatuses,
		lateAnswer,
		mergeSuggestions,
		missingRequired,
		newDraftKey,
		parseDraft,
		photoLock,
		startsAnotherOrder,
		toNewOrder,
		withPrefill,
		type IntakeDraft,
		type RequiredKey
	} from './draft.js';
	import IntakeAside from './IntakeAside.svelte';
	import IntakePhotos from './IntakePhotos.svelte';
	import { intake } from './intake.svelte.js';

	const t = ru.intake;
	/** Pause after the last key before the draft is written and the suggestions are asked. */
	const PAUSE_MS = 300;
	/** Chips of «Частые неисправности»: the first ones of the dictionary. */
	const FREQUENT = 12;

	type MoneyKey = 'estimateFrom' | 'estimateTo' | 'prepayment';
	const MONEY_FIELDS: Record<MoneyKey, { label: string; id: string }> = {
		estimateFrom: { label: t.estimateFrom, id: 'intake-estimate-from' },
		estimateTo: { label: t.estimateTo, id: 'intake-estimate-to' },
		prepayment: { label: t.prepayment, id: 'intake-prepayment' }
	};
	const MONEY_KEYS = Object.keys(MONEY_FIELDS) as MoneyKey[];

	/** The fields that can show an error, in the order of the form, with the element to focus. */
	const FIELDS = {
		contact: PHONE_ID,
		phones: PHONE_ID,
		client: PHONE_ID,
		deviceType: 'intake-type',
		brand: 'intake-brand',
		model: 'intake-model',
		malfunction: 'intake-malfunction',
		status: 'intake-status',
		estimateFrom: MONEY_FIELDS.estimateFrom.id,
		estimateTo: MONEY_FIELDS.estimateTo.id,
		prepayment: MONEY_FIELDS.prepayment.id
	} as const;
	type FieldKey = keyof typeof FIELDS;
	/** The fields of the server's validation errors. */
	const SERVER_FIELDS: Record<string, FieldKey> = {
		full_name: 'contact',
		phones: 'phones',
		client_id: 'client',
		device_type: 'deviceType',
		brand: 'brand',
		model: 'model',
		malfunction: 'malfunction',
		status_id: 'status',
		estimate_from: 'estimateFrom',
		estimate_to: 'estimateTo',
		prepayment: 'prepayment'
	};

	const KINDS = [
		'device_type',
		'brand',
		'color',
		'completeness',
		'condition',
		'malfunction',
		'master',
		'client_source',
		'warranty'
	] as const satisfies readonly DictionaryKind[];
	type Kind = (typeof KINDS)[number];
	const noDictionaries = () =>
		Object.fromEntries(KINDS.map((k) => [k, [] as string[]])) as Record<Kind, string[]>;

	let draft = $state<IntakeDraft>(emptyDraft(app.settings, app.statuses));
	let dictionaries = $state.raw(noDictionaries());
	let attempted = $state(false);
	let submitError = $state<string | null>(null);
	/** The draft as it was when `submitError` was shown: any edit since then hides it. */
	let errorFor = '';
	/**
	 * The photos of the draft. Kept while the sheet is closed: uploads on their way go on and show again
	 * at the next opening of the same draft.
	 */
	let media = $state.raw<Media | null>(null);
	let mediaKey = '';
	let serverErrors = $state.raw<Partial<Record<FieldKey, string>>>({});
	/** Money fields holding text that is not an amount: the order is not accepted until they are fixed. */
	let wrongMoney = $state<Record<MoneyKey, boolean>>({
		estimateFrom: false,
		estimateTo: false,
		prepayment: false
	});

	// The draft is written a moment after the last change; closing writes it at once.
	let written = '';
	let unsaved: string | null = null;
	let saveTimer: ReturnType<typeof setTimeout> | undefined;

	// Every opening starts from the draft of this device (and the client asked for), with fresh dictionaries.
	let wasOpen = false;
	$effect.pre(() => {
		const open = intake.isOpen;
		untrack(() => {
			if (open && !wasOpen) begin();
			wasOpen = open;
		});
	});

	function begin() {
		const stored = parseDraft(readStorage(INTAKE_DRAFT_KEY), app.settings, app.statuses);
		const next = withPrefill(stored, intake.prefill, app.settings, app.statuses);
		written = JSON.stringify(next);
		// A draft already accepted is replaced at once, not at the first key: it is not offered again.
		if (next !== stored && stored.acceptedAs !== null) writeStorage(INTAKE_DRAFT_KEY, written);
		draft = next;
		attempted = false;
		submitError = null;
		serverErrors = {};
		wrongMoney = { estimateFrom: false, estimateTo: false, prepayment: false };
		loadDictionaries();
		photosOf(draft.key);
	}

	/** The photos of the draft with this key, read again from the server. */
	function photosOf(key: string) {
		if (media !== null && mediaKey === key) {
			void media.reload();
			return;
		}
		media = createDraftMedia(key);
		mediaKey = key;
	}

	/** The draft becomes another one (another client, an attempt the server refuses): a new key. */
	function renew() {
		draft.key = newDraftKey();
		draft.attempt = null;
		draft.acceptedAs = null;
		photosOf(draft.key);
	}

	/**
	 * «Принять» of `form` got no answer in time: the server may have made the order. The draft remembers
	 * what that first attempt asked for, here or in the storage when the sheet is closed.
	 */
	function unanswered(form: IntakeDraft) {
		const attempt = attemptOf(form);
		if (intake.isOpen) {
			if (draft.key === form.key) draft.attempt ??= attempt;
			return;
		}
		const json = readStorage(INTAKE_DRAFT_KEY);
		const stored = json === null ? null : parseDraft(json, app.settings, app.statuses);
		if (stored?.key === form.key && stored.attempt === null)
			writeStorage(INTAKE_DRAFT_KEY, JSON.stringify({ ...stored, attempt }));
	}

	/** Photos are on their way: the draft that holds their key is written at once. */
	function keep() {
		unsaved = JSON.stringify(draft);
		flush();
	}

	function showError(message: string) {
		submitError = message;
		errorFor = JSON.stringify(draft);
	}

	let dictionarySeq = 0;
	function loadDictionaries() {
		const mine = ++dictionarySeq;
		void Promise.all(KINDS.map((k) => listDictionary(k).catch(() => []))).then((lists) => {
			// Without them the fields still take any text.
			if (mine !== dictionarySeq) return;
			dictionaries = Object.fromEntries(
				KINDS.map((k, i) => [k, (lists[i] ?? []).map((d) => d.value)])
			) as Record<Kind, string[]>;
		});
	}

	$effect(() => {
		if (!intake.isOpen) return;
		const json = JSON.stringify(draft);
		untrack(() => {
			// An error of the server is about the old value: editing hides it.
			if (Object.keys(serverErrors).length > 0) serverErrors = {};
			if (submitError !== null && json !== errorFor) submitError = null;
			if (json === written) return;
			unsaved = json;
			clearTimeout(saveTimer);
			saveTimer = setTimeout(flush, PAUSE_MS);
		});
	});
	function flush() {
		clearTimeout(saveTimer);
		if (unsaved === null) return;
		// Storage unavailable: the draft lives as long as the page.
		writeStorage(INTAKE_DRAFT_KEY, unsaved);
		written = unsaved;
		unsaved = null;
	}
	function discard() {
		clearTimeout(saveTimer);
		unsaved = null;
		written = '';
		removeStorage(INTAKE_DRAFT_KEY);
	}
	/**
	 * The answer to an order sent before Esc came when the sheet was closed, or closed and opened again.
	 * The very draft that was accepted goes, and the sheet with it. The same draft (its key) changed since
	 * then stays, marked as accepted: the sheet says so at every opening until «Отмена», «Принять» or
	 * another client. A draft begun meanwhile is left alone.
	 */
	function settleLate(sent: string, order: Order) {
		const open = intake.isOpen;
		const json = open ? JSON.stringify(draft) : readStorage(INTAKE_DRAFT_KEY);
		if (json === null) return;
		const now = parseDraft(json, app.settings, app.statuses);
		const before = parseDraft(sent, app.settings, app.statuses);
		const answer = lateAnswer(now, before);
		if (answer === 'close') {
			discard();
			if (open) intake.close();
			return;
		}
		if (answer === 'other') return;
		const mark = { id: order.id, key: before.key, clientId: order.client_id };
		const attempt = now.attempt ?? attemptOf(before);
		if (open) {
			draft.attempt = attempt;
			draft.acceptedAs = mark;
			// Its photos are the order's now.
			void media?.reload();
		} else writeStorage(INTAKE_DRAFT_KEY, JSON.stringify({ ...now, attempt, acceptedAs: mark }));
	}
	$effect(() => {
		window.addEventListener('pagehide', flush);
		return () => {
			window.removeEventListener('pagehide', flush);
			flush();
		};
	});

	/** Esc, ✕, an order of the right column: the sheet closes, the draft waits for the next opening. */
	function leave() {
		if (!intake.isOpen) return;
		flush();
		intake.close();
	}
	/**
	 * The draft goes, and its photos on the server once the uploads on their way are done. Without a
	 * connection the server removes them itself a day later.
	 */
	function cancel() {
		// Still waiting for its photos: the order is not sent.
		intake.callOff();
		const key = draft.key;
		const photos = media;
		void (photos?.idle() ?? Promise.resolve()).then(() => discardDraft(key)).catch(() => {});
		discard();
		intake.close();
	}

	/** Another client than the one an earlier «Принять» was for: a new order of the same form. */
	function chosen(client: Client) {
		if (startsAnotherOrder(draft, client)) renew();
		void focusField('deviceType');
	}
	function openOrder(id: number) {
		leave();
		void orderPanel.open(id);
	}

	// Suggestions: the dictionary, then the values of past orders, asked a moment after the last key.
	function suggestions(field: OrderValueField, text: () => string, brand?: () => string) {
		let values = $state.raw<string[]>([]);
		$effect(() => {
			if (!intake.isOpen) return;
			const query = text().trim();
			const of = brand?.().trim() || undefined;
			const controller = new AbortController();
			const timer = setTimeout(() => {
				orderValues(field, query, of, controller.signal).then(
					(v) => (values = v),
					() => {}
				);
			}, PAUSE_MS);
			return () => {
				clearTimeout(timer);
				controller.abort();
			};
		});
		return {
			get values() {
				return values;
			}
		};
	}
	const types = suggestions('device_type', () => draft.deviceType);
	const brands = suggestions('brand', () => draft.brand);
	const models = suggestions(
		'model',
		() => draft.model,
		() => draft.brand
	);
	const items = (values: string[]) => values.map((value) => ({ value }));
	const typeItems = $derived(items(mergeSuggestions(dictionaries.device_type, types.values)));
	const brandItems = $derived(items(mergeSuggestions(dictionaries.brand, brands.values)));
	const modelItems = $derived(items(models.values));
	const colorItems = $derived(items(dictionaries.color));
	const masterItems = $derived(items(dictionaries.master));
	const warrantyItems = $derived(
		items(mergeSuggestions(dictionaries.warranty, [app.settings.default_warranty]))
	);
	// Chips of the draft stay on screen even when the dictionary no longer has them.
	const completeness = $derived(mergeSuggestions(dictionaries.completeness, draft.completeness));
	const condition = $derived(mergeSuggestions(dictionaries.condition, draft.condition));
	const frequent = $derived(
		mergeSuggestions(dictionaries.malfunction.slice(0, FREQUENT), draft.malfunctionChips)
	);
	const statusItems = $derived(
		intakeStatuses(app.statuses).map((s) => ({ value: s.id, label: s.name }))
	);

	/** «+ своё» goes into the order, and into the dictionary when the user may fill it. */
	function remember(kind: Kind, value: string) {
		if (!app.can('settings_manage') && !app.can('orders_create')) return;
		// Not added to the dictionary: the value is in the order anyway.
		addDictionaryItem(kind, value).catch(() => {});
	}

	/** «intake_required_fields»: the server also wants the brand and the model. */
	const strict = $derived(app.settings.intake_required_fields);
	const missing = $derived(missingRequired(draft, strict));
	const wrong = $derived(MONEY_KEYS.filter((k) => wrongMoney[k]));
	const required = (key: RequiredKey, text: string) =>
		attempted && missing.includes(key) ? text : undefined;
	const errors = $derived({
		...serverErrors,
		contact: serverErrors.contact ?? required('contact', t.contactRequired),
		deviceType: serverErrors.deviceType ?? required('deviceType', t.deviceTypeRequired),
		brand: serverErrors.brand ?? required('brand', t.brandRequired),
		model: serverErrors.model ?? required('model', t.modelRequired),
		malfunction: serverErrors.malfunction ?? required('malfunction', t.malfunctionRequired)
	});

	/** The order belongs to the point of the top bar, otherwise to the default one of the settings. */
	const point = $derived(app.point ?? app.settings.default_service_point_id);

	async function focusField(key: FieldKey | MoneyKey) {
		await tick();
		document.getElementById(FIELDS[key])?.focus();
	}

	/**
	 * The order goes once: while it is on its way «Принять» waits, here and in the sheet opened again
	 * after Esc. Its answer closes and clears only the opening that sent it.
	 */
	async function accept() {
		if (intake.submitting) return;
		if (missing.length > 0 || wrong.length > 0) {
			attempted = true;
			// The first field to fix, in the order of the form: the required ones come before the money.
			const first = missing[0] ?? wrong[0];
			if (first) void focusField(first);
			return;
		}
		submitError = null;
		serverErrors = {};
		// What is sent is what this device keeps: Esc right now leaves the same draft.
		flush();
		const sent = JSON.stringify(draft);
		const form = $state.snapshot(draft);
		const pointId = point;
		const photos = media;
		// The photos still on their way and the list of the draft come first («Загружаю фото…»): the
		// order takes what the draft has. A list that did not load may have some, so the draft goes then too.
		const withPhotos = () =>
			photos !== null && (photos.photos.length > 0 || photos.error !== null || photos.loading);
		// The form is inert until the answer (`intake.formLocked`): the field it was sent from loses focus.
		const from = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		const answer = await intake.submit(
			(signal) => createOrder(toNewOrder(form, pointId, withPhotos()), signal),
			{ ready: photos === null ? undefined : () => photos.idle() }
		);
		void refocus(from);
		if (answer === null) return;
		if (!answer.ok) {
			refused(answer.error, answer.current, answer.timedOut, form);
			return;
		}
		const order: Order = answer.value;
		emitOrderChanged(order, 'created');
		if (!answer.current) {
			settleLate(sent, order);
		} else if (draft.key !== form.key) {
			// Another order is typed meanwhile (another client): it stays, the toast tells about this one.
		} else if (acceptedAnswer(draft) === 'keep') {
			// The order an earlier attempt made: the changes since are not in it, the sheet says so.
			draft.acceptedAs = { id: order.id, key: form.key, clientId: order.client_id };
			return;
		} else {
			discard();
			intake.close();
		}
		// Toasts cannot be used under a modal: this one waits for the sheet to go; a sheet opened again
		// and changed meanwhile says it above «Принять».
		await tick();
		const id = order.id;
		toast.show({
			message: t.accepted(id),
			tone: 'success',
			action: { label: t.open, run: () => void orderPanel.open(id) }
		});
	}

	/** Focus lost while the form was inert goes back where it was, if the sheet is still open. */
	async function refocus(el: HTMLElement | null) {
		await tick();
		if (!intake.isOpen || el === null || !el.isConnected) return;
		const now = document.activeElement;
		if (now === null || now === document.body || now === el.closest('[role="dialog"]'))
			el.focus({ preventScroll: true });
	}

	const lowerFirst = (text: string) => text.charAt(0).toLowerCase() + text.slice(1);

	/**
	 * Errors of the fields go to the fields; anything else (no connection) above «Принять». An answer
	 * about an earlier opening only says why, above «Принять» or in a toast once the sheet is closed.
	 */
	function refused(e: unknown, current: boolean, timedOut: boolean, form: IntakeDraft) {
		// No answer in time: the server may have made the order all the same.
		if (timedOut) unanswered(form);
		if (timedOut || !current) {
			const message = timedOut ? t.noAnswer : t.failed(lowerFirst(errorMessage(e)));
			// Closed while it was on its way: the draft waits, the toast says why.
			if (intake.isOpen) showError(message);
			else toast.show({ message, tone: 'danger' });
			return;
		}
		const fields = e instanceof ApiError && e.code === 'validation' ? (e.body?.fields ?? []) : [];
		// The key of this attempt belongs to an order deleted since: the next «Принять» is a new attempt.
		if (fields.some((f) => f.field === 'request_key')) renew();
		const found: Partial<Record<FieldKey, string>> = {};
		const other: string[] = [];
		for (const f of fields) {
			const key = SERVER_FIELDS[f.field];
			if (key === undefined) other.push(f.message);
			else found[key] ??= f.message;
		}
		serverErrors = found;
		const first = (Object.keys(FIELDS) as FieldKey[]).find((k) => found[k] !== undefined);
		if (first) void focusField(first);
		if (other.length > 0 || fields.length === 0) {
			const message = other.length > 0 ? other.join('; ') : errorMessage(e);
			showError(t.failed(lowerFirst(message)));
		}
	}

	$effect(() => {
		if (intake.isOpen) return submitShortcut(() => void accept());
	});
</script>

<Sheet
	bind:open={
		() => intake.isOpen,
		(v) => {
			if (!v) leave();
		}
	}
	title={t.title}
	onclose={leave}
>
	<div class="layout">
		<div class="form" inert={intake.formLocked}>
			<ClientStep bind:draft sources={dictionaries.client_source} {errors} onchosen={chosen} />

			<Section title={t.device}>
				<div class="grid three">
					<Combobox
						id={FIELDS.deviceType}
						label={t.deviceType}
						required
						items={typeItems}
						bind:value={draft.deviceType}
						error={errors.deviceType}
					/>
					<Combobox
						id={FIELDS.brand}
						label={t.brand}
						required={strict}
						items={brandItems}
						bind:value={draft.brand}
						error={errors.brand}
					/>
					<Combobox
						id={FIELDS.model}
						label={t.model}
						required={strict}
						items={modelItems}
						bind:value={draft.model}
						error={errors.model}
					/>
					<Combobox label={t.color} items={colorItems} bind:value={draft.color} />
					<TextInput label={t.serial} autocomplete="off" bind:value={draft.serial} />
					<TextInput
						label={t.password}
						type="password"
						revealable
						autocomplete="off"
						bind:value={draft.password}
					/>
				</div>
				<ChipGroup
					label={t.completeness}
					options={completeness}
					allowCustom
					bind:selected={draft.completeness}
					oncreate={(v) => remember('completeness', v)}
				/>
				<ChipGroup
					label={t.condition}
					options={condition}
					allowCustom
					bind:selected={draft.condition}
					oncreate={(v) => remember('condition', v)}
				/>
			</Section>

			<Section title={t.malfunction}>
				<Textarea
					id={FIELDS.malfunction}
					label={t.malfunction}
					required
					bind:value={draft.malfunction}
					error={errors.malfunction}
				/>
				{#if frequent.length > 0}
					<ChipGroup label={t.frequent} options={frequent} bind:selected={draft.malfunctionChips} />
				{/if}
			</Section>

			{#if media}
				<IntakePhotos
					{media}
					locked={intake.submitting || photoLock(draft) !== null}
					reason={photoLock(draft)}
					onadd={keep}
				/>
			{/if}

			<Section title={t.work}>
				<div class="grid three">
					<Combobox label={t.master} items={masterItems} bind:value={draft.master} />
					<Select
						id={FIELDS.status}
						label={t.status}
						items={statusItems}
						bind:value={draft.statusId}
						error={errors.status}
					/>
					<Combobox label={t.warranty} items={warrantyItems} bind:value={draft.warranty} />
					{#each MONEY_KEYS as key (key)}
						<MoneyInput
							id={MONEY_FIELDS[key].id}
							label={MONEY_FIELDS[key].label}
							bind:value={draft[key]}
							bind:invalid={wrongMoney[key]}
							error={errors[key]}
						/>
					{/each}
				</div>
				<Textarea label={t.comment} bind:value={draft.comment} />
			</Section>
		</div>

		<IntakeAside
			{missing}
			wrong={wrong.map((k) => MONEY_FIELDS[k].label)}
			clientId={draft.clientId}
			serial={draft.serial}
			onopen={openOrder}
		/>
	</div>

	{#snippet footer()}
		{#if submitError}<p class="failed" role="alert">{submitError}</p>{/if}
		{#if draft.acceptedAs !== null}
			{@const id = draft.acceptedAs.id}
			<p class="note" role="status">
				{t.acceptedMeanwhile(id)}
				<Button size="sm" variant="ghost" onclick={() => openOrder(id)}>{t.open}</Button>
			</p>
		{/if}
		{#if !app.me.open_mode}<span class="receiver">{t.receiver(app.me.name)}</span>{/if}
		{#if intake.uploading}
			<span class="sending" role="status">{t.uploadingPhotos}</span>
		{:else if intake.submitting}
			<span class="sending" role="status">{t.accepting}</span>
		{/if}
		<!-- The order on its way cannot be called back: «Отмена» waits for its answer. While it waits for
		     its photos it has not gone yet. -->
		<Button disabled={intake.submitting && !intake.uploading} onclick={cancel}>{t.cancel}</Button>
		<Button
			variant="primary"
			shortcut="Mod+Enter"
			busy={intake.submitting}
			onclick={() => void accept()}>{t.accept}</Button
		>
	{/snippet}
</Sheet>

<style>
	.layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(14rem, 18rem);
		align-items: start;
		gap: var(--space-5);
	}
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
		min-width: 0;
	}
	.grid {
		display: grid;
		gap: var(--space-3);
	}
	.grid.three {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}
	.failed,
	.note {
		flex: 1 0 100%;
		margin: 0;
		text-align: end;
	}
	.failed {
		color: var(--danger-text);
	}
	.sending {
		color: var(--text-3);
	}
	.receiver {
		margin-right: auto;
		color: var(--text-3);
	}
	:global(:is([data-width='xs'], [data-width='sm'])) .layout {
		grid-template-columns: minmax(0, 1fr);
	}
	:global(:is([data-width='xs'], [data-width='sm'])) .grid.three {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}
	:global([data-width='xs']) .grid.three {
		grid-template-columns: minmax(0, 1fr);
	}
</style>
