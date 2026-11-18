<!-- «Выдать» (spec 6.4): what is required is filled in right here, then the sum to pay; Ctrl/⌘+Enter
     issues. The fields the dialog changed are saved first, then the order is issued. -->
<script lang="ts" module>
	/** What the dialog fills in before the order is issued. */
	export interface IssueValues {
		work_done: string;
		master: string;
		final_price: number;
		discount: number;
		warranty: string;
	}
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import type { Order } from '$api/Order';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { formatMoney } from '#lib/format/money.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import MoneyInput from '#lib/ui/MoneyInput.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import { isNoWarranty, issueWarranty, toPay } from './view.js';

	let {
		open = $bindable(false),
		order,
		dictionaries,
		defaultWarranty,
		onissue
	}: {
		open: boolean;
		/** The order as the panel shows it, own unsaved values included. */
		order: Order | null;
		dictionaries: { master: string[]; warranty: string[] };
		defaultWarranty: string;
		/** Saves and issues; throws the server's error, which the dialog shows. */
		onissue: (values: IssueValues) => Promise<void>;
	} = $props();

	const t = ru.order;
	type Key = 'work' | 'master' | 'price' | 'discount' | 'warranty';
	const IDS: Record<Key, string> = {
		work: 'issue-work',
		master: 'issue-master',
		price: 'issue-price',
		discount: 'issue-discount',
		warranty: 'issue-warranty'
	};
	/** Fields of the server's errors and where they are shown. */
	const SERVER_FIELDS: Record<string, Key> = {
		work_done: 'work',
		master: 'master',
		final_price: 'price',
		discount: 'discount',
		warranty: 'warranty'
	};
	const masterItems = $derived(dictionaries.master.map((value) => ({ value })));
	const warrantyItems = $derived(dictionaries.warranty.map((value) => ({ value })));

	let work = $state('');
	let master = $state('');
	let price = $state<number | null>(null);
	let discount = $state<number | null>(0);
	let warranty = $state('');
	/** The money fields hold text that is not an amount: nothing is issued until it is fixed. */
	let priceInvalid = $state(false);
	let discountInvalid = $state(false);
	let serverErrors = $state<Partial<Record<Key, string>>>({});
	let failure = $state<string | null>(null);
	let busy = $state(false);

	// Every opening starts from the order as it is now.
	$effect.pre(() => {
		if (!open) return;
		untrack(() => {
			priceInvalid = false;
			discountInvalid = false;
			serverErrors = {};
			failure = null;
			busy = false;
			if (!order) return;
			work = order.work_done;
			master = order.master;
			price = order.final_price;
			discount = order.discount;
			warranty = issueWarranty(order, defaultWarranty);
		});
	});

	const missing = $derived({
		work: work.trim() === '',
		master: master.trim() === '',
		price: price === null
	});
	/** What stops the issue, in the order of the fields: an empty required field or a wrong amount. */
	const blocking = $derived<Record<Key, boolean>>({
		work: missing.work,
		master: missing.master,
		price: missing.price || priceInvalid,
		discount: discountInvalid,
		warranty: false
	});
	const wrong = $derived([
		...(priceInvalid ? [t.issuePrice] : []),
		...(discountInvalid ? [t.discount] : [])
	]);
	// A wrong amount leaves the sum unknown.
	const pay = $derived(
		order && wrong.length === 0
			? toPay({ final_price: price, prepayment: order.prepayment, discount: discount ?? 0 })
			: null
	);
	const payLine = $derived(`${t.toPay} ${pay === null ? '—' : formatMoney(pay)}`);
	const noWarranty = $derived(order !== null && isNoWarranty(order) && warranty === order.warranty);

	function drop(k: Key) {
		if (serverErrors[k]) serverErrors = { ...serverErrors, [k]: undefined };
	}

	async function issue() {
		if (busy || !order) return;
		const first = (Object.keys(blocking) as Key[]).find((k) => blocking[k]);
		if (first !== undefined) {
			document.getElementById(IDS[first])?.focus();
			return;
		}
		if (price === null) return;
		busy = true;
		serverErrors = {};
		failure = null;
		try {
			await onissue({
				work_done: work.trim(),
				master: master.trim(),
				final_price: price,
				discount: discount ?? 0,
				warranty: warranty.trim()
			});
		} catch (e) {
			const fields = e instanceof ApiError && e.code === 'validation' ? (e.body?.fields ?? []) : [];
			const shown: Partial<Record<Key, string>> = {};
			for (const f of fields) {
				const k = SERVER_FIELDS[f.field];
				if (k) shown[k] ??= f.message;
			}
			serverErrors = shown;
			const known = Object.keys(shown) as Key[];
			if (known.length > 0) document.getElementById(IDS[known[0]!])?.focus();
			else failure = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		if (open) return submitShortcut(() => void issue());
	});
</script>

<Dialog bind:open title={t.issueTitle(order?.id ?? 0)}>
	{#if order}
		<div class="issue">
			{#if failure}<Callout tone="danger">{failure}</Callout>{/if}
			<Textarea
				id={IDS.work}
				label={t.workDone}
				required
				bind:value={
					() => work,
					(v) => {
						work = v;
						drop('work');
					}
				}
				error={missing.work ? t.required : serverErrors.work}
			/>
			<Combobox
				id={IDS.master}
				label={t.master}
				required
				items={masterItems}
				bind:value={
					() => master,
					(v) => {
						master = v;
						drop('master');
					}
				}
				error={missing.master ? t.required : serverErrors.master}
			/>
			<div class="pair">
				<MoneyInput
					id={IDS.price}
					label={t.issuePrice}
					required
					bind:value={
						() => price,
						(v) => {
							price = v;
							drop('price');
						}
					}
					bind:invalid={priceInvalid}
					error={missing.price && !priceInvalid ? t.required : serverErrors.price}
				/>
				<MoneyInput
					id={IDS.discount}
					label={t.discount}
					nullable={false}
					bind:value={
						() => discount,
						(v) => {
							discount = v;
							drop('discount');
						}
					}
					bind:invalid={discountInvalid}
					error={serverErrors.discount}
				/>
			</div>
			<div class="sum num">
				<p class="to-pay" class:negative={pay !== null && pay < 0}>{payLine}</p>
				{#if wrong.length > 0}
					<p class="wrong">{t.wrongSum(wrong.join(', '))}</p>
				{:else}
					<p class="muted">
						{t.finalPrice}
						{price === null ? '—' : formatMoney(price)} − {t.prepayment.toLowerCase()}
						{formatMoney(order.prepayment)} − {t.discount.toLowerCase()}
						{formatMoney(discount ?? 0)}
					</p>
				{/if}
			</div>
			<Combobox
				id={IDS.warranty}
				label={t.warranty}
				items={warrantyItems}
				hint={noWarranty ? t.noWarranty : undefined}
				bind:value={
					() => warranty,
					(v) => {
						warranty = v;
						drop('warranty');
					}
				}
				error={serverErrors.warranty}
			/>
		</div>
	{/if}
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" shortcut="Mod+Enter" {busy} onclick={() => void issue()}
			>{t.issue}</Button
		>
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
	:global([data-width='xs']) .pair {
		grid-template-columns: minmax(0, 1fr);
	}
</style>
