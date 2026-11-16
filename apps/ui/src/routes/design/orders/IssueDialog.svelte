<!-- «Выдать» (spec 6.4): what is required first, then the sum to pay; Ctrl/⌘+Enter issues. -->
<script lang="ts" module>
	/** What the dialog fills in before the order is issued. */
	export interface IssuePatch {
		work_done: string;
		master: string;
		final_price: number;
		discount: number;
		warranty: string;
	}
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { readStorage, writeStorage } from '#lib/design/storage.js';
	import { makeDictionaries } from '#lib/demo/fake.js';
	import { toPay, type DemoOrder } from '#lib/demo/orders.js';
	import { displayName, formatMoney } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import { Button, Dialog, MoneyInput, Select, Switch, Textarea } from '#lib/ui/index.js';

	let {
		open = $bindable(false),
		order,
		onissue
	}: {
		open: boolean;
		order: DemoOrder | undefined;
		onissue: (patch: IssuePatch, printAct: boolean) => void;
	} = $props();

	/** «Печатать акт выдачи» is remembered on this device. */
	const PRINT_KEY = 'mw.demo.printIssueAct';
	const REQUIRED = 'Заполните перед выдачей';
	const DICTIONARIES = makeDictionaries();
	const masterItems = DICTIONARIES.master.map((m) => ({ value: m, label: displayName(m) }));
	const warrantyItems = DICTIONARIES.warranty.map((w) => ({ value: w, label: w }));
	const ids = {
		work: 'issue-work',
		master: 'issue-master',
		price: 'issue-price',
		discount: 'issue-discount'
	} as const;
	const LABELS = { price: 'Итоговая цена', discount: 'Скидка' } as const;

	let work = $state('');
	let master = $state<string | null>(null);
	let price = $state<number | null>(null);
	let discount = $state<number | null>(0);
	let warranty = $state<string | null>(null);
	let printAct = $state(readPrintAct());
	/** The money fields hold text that is not an amount: nothing is issued until it is fixed. */
	let priceInvalid = $state(false);
	let discountInvalid = $state(false);

	function readPrintAct(): boolean {
		return readStorage(PRINT_KEY) !== 'false';
	}
	function rememberPrintAct(value: boolean) {
		printAct = value;
		// storage unavailable: the switch works for this session
		writeStorage(PRINT_KEY, String(value));
	}

	// Every opening starts from the order as it is now.
	$effect.pre(() => {
		if (!open) return;
		untrack(() => {
			priceInvalid = false;
			discountInvalid = false;
			if (!order) return;
			work = order.work_done;
			master = order.master || null;
			price = order.final_price;
			discount = order.discount;
			warranty = order.warranty || null;
		});
	});

	const missing = $derived({
		work: work.trim() === '',
		master: master === null,
		price: price === null
	});
	/** What stops the issue, in the order of the fields: an empty required field or a wrong amount. */
	const blocking = $derived({
		work: missing.work,
		master: missing.master,
		price: missing.price || priceInvalid,
		discount: discountInvalid
	});
	const wrong = $derived([
		...(priceInvalid ? [LABELS.price] : []),
		...(discountInvalid ? [LABELS.discount] : [])
	]);
	// A wrong amount leaves the sum unknown.
	const pay = $derived(
		order && wrong.length === 0
			? toPay({ final_price: price, prepayment: order.prepayment, discount: discount ?? 0 })
			: null
	);
	const payText = $derived(pay === null ? '—' : formatMoney(pay));

	function issue() {
		const first = (Object.keys(blocking) as (keyof typeof blocking)[]).find((k) => blocking[k]);
		if (first !== undefined) {
			document.getElementById(ids[first])?.focus();
			return;
		}
		if (master === null || price === null) return;
		open = false;
		onissue(
			{
				work_done: work.trim(),
				master,
				final_price: price,
				discount: discount ?? 0,
				warranty: warranty ?? ''
			},
			printAct
		);
	}

	$effect(() => {
		if (open) return submitShortcut(issue);
	});
</script>

<Dialog bind:open title="Выдача заказа № {order?.id ?? ''}">
	{#if order}
		<div class="issue">
			<Textarea
				id={ids.work}
				label="Выполненные работы"
				required
				bind:value={work}
				error={missing.work ? REQUIRED : undefined}
			/>
			<Select
				id={ids.master}
				label="Мастер"
				required
				items={masterItems}
				bind:value={master}
				error={missing.master ? REQUIRED : undefined}
			/>
			<div class="pair">
				<MoneyInput
					id={ids.price}
					label={LABELS.price}
					required
					bind:value={price}
					bind:invalid={priceInvalid}
					error={missing.price && !priceInvalid ? REQUIRED : undefined}
				/>
				<MoneyInput
					id={ids.discount}
					label={LABELS.discount}
					nullable={false}
					bind:value={discount}
					bind:invalid={discountInvalid}
				/>
			</div>
			<div class="sum num">
				<p class="to-pay" class:negative={pay !== null && pay < 0}>К оплате {payText}</p>
				{#if wrong.length > 0}
					<p class="wrong">Неверная сумма: {wrong.join(', ')}</p>
				{:else}
					<p class="muted">
						Итог {price === null ? '—' : formatMoney(price)} − предоплата {formatMoney(
							order.prepayment
						)}
						− скидка {formatMoney(discount ?? 0)}
					</p>
				{/if}
			</div>
			<Select label="Гарантия" items={warrantyItems} bind:value={warranty} />
			<Switch
				label="Печатать акт выдачи"
				hint="Запоминается на этом устройстве"
				bind:checked={() => printAct, rememberPrintAct}
			/>
		</div>
	{/if}
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" shortcut="Mod+Enter" onclick={issue}>Выдать</Button>
	{/snippet}
</Dialog>

<style>
	.issue {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.pair {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-3);
	}
	.sum {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}
	.sum p {
		margin: 0;
	}
	.to-pay {
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.to-pay.negative {
		color: var(--danger-text);
	}
	.muted {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.wrong {
		color: var(--warning-text);
		font-size: var(--text-sm);
	}
</style>
