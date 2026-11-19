<!-- Merging clients (spec 6.6): a column for every client, who stays, which client every field comes from,
     the phones joined, and what is left. «Объединить» asks once more: it cannot be undone. A client changed
     meanwhile is a 409: the dialog says so and reads everyone again, keeping the choices still possible. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { Client } from '$api/Client';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { getClient, mergeClients } from '#lib/api/clients.js';
	import { listOrders } from '#lib/api/orders.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { countLabel, displayName, formatPhone, plural } from '#lib/format/index.js';
	import { phoneKey } from '#lib/format/phone.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import {
		defaultChoice,
		keepChoice,
		keptMergedPhones,
		MAX_CLIENT_PHONES,
		mergedPhones,
		toMergeRequest,
		type MergeChoice,
		type MergeClient,
		type MergeField
	} from './merge.js';

	let {
		open = $bindable(false),
		ids,
		counts = {},
		onmerged
	}: {
		open: boolean;
		/** The clients to merge (2..=10); the dialog reads them itself: it needs their versions. */
		ids: number[];
		/** The numbers of orders the list already knows (by client): only the others are asked for. */
		counts?: Record<number, number>;
		/** The kept client, as the server returned them. */
		onmerged: (c: Client) => void;
	} = $props();

	const t = ru.merge;
	const uid = $props.id();

	let clients = $state.raw<MergeClient[] | null>(null);
	let choice = $state.raw<MergeChoice | null>(null);
	let loadError = $state<string | null>(null);
	let stale = $state(false);
	let confirming = $state(false);
	/** The numbers the user unchecked (`phoneKey`s): they are left out of the merged client. */
	let dropped = $state.raw(new Set<string>());
	let token = 0;

	/** Numbers of orders from the list; a client changed meanwhile is read again with a fresh count. */
	async function readAll(known: Record<number, number>): Promise<MergeClient[]> {
		return Promise.all(
			ids.map(async (id) => {
				const have = known[id];
				if (have !== undefined) return { ...(await getClient(id)), orders_count: have };
				const [c, orders] = await Promise.all([
					getClient(id),
					listOrders({ client_id: id, limit: 1 })
				]);
				return { ...c, orders_count: orders.total };
			})
		);
	}

	async function load(keep: boolean) {
		const mine = ++token;
		loadError = null;
		try {
			const read = await readAll(keep ? {} : counts);
			if (mine !== token) return;
			const previous = choice;
			clients = read;
			choice = keep && previous ? keepChoice(previous, read) : defaultChoice(read);
		} catch (e) {
			if (mine !== token) return;
			loadError = errorMessage(e);
		}
	}

	$effect(() => {
		if (!open) return;
		untrack(() => {
			clients = null;
			choice = null;
			stale = false;
			confirming = false;
			dropped = new Set();
			void load(false);
		});
		return () => {
			token++;
		};
	});

	$effect(() => {
		if (open && !confirming && clients && choice) return submitShortcut(() => (confirming = true));
	});

	const FIELD_ROWS: { field: MergeField; label: string }[] = [
		{ field: 'full_name', label: ru.client.fullName },
		{ field: 'address', label: ru.client.address },
		{ field: 'source', label: ru.client.source },
		{ field: 'note', label: ru.client.note },
		{ field: 'blacklisted', label: ru.client.blacklisted }
	];

	const phonesOf = (c: Client): string[] =>
		c.phones.length === 0
			? [t.empty]
			: c.phones.map((p) =>
					p.note ? `${formatPhone(p.phone)} (${p.note})` : formatPhone(p.phone)
				);
	function shown(c: Client, f: MergeField): string {
		if (f === 'blacklisted') return c.blacklisted ? t.yes : t.no;
		if (f === 'full_name') return displayName(c.full_name) || t.empty;
		return c[f] || t.empty;
	}

	const request = $derived(clients && choice ? toMergeRequest(choice, clients, dropped) : null);
	/** Every number of the clients joined; more than a client may have: some have to be left out. */
	const joined = $derived(clients && choice ? mergedPhones(clients, choice.targetId) : []);
	const kept = $derived(joined.filter((p) => !dropped.has(phoneKey(p.phone))));
	const tooMany = $derived(kept.length > MAX_CLIENT_PHONES);
	function keepPhone(phone: string, keep: boolean) {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a copy that replaces the raw state
		const next = new Set(dropped);
		if (keep) next.delete(phoneKey(phone));
		else next.add(phoneKey(phone));
		dropped = next;
	}
	const summary = $derived.by(() => {
		if (!clients || !choice || !request) return '';
		const phones = keptMergedPhones(clients, choice.targetId, dropped);
		const orders = clients.reduce((sum, c) => sum + c.orders_count, 0);
		return [
			`${t.stays}: ${displayName(request.full_name) || ru.client.noName}`,
			countLabel(orders, ru.orders.forms),
			phones.length === 0 ? t.noPhones : phones.map((p) => formatPhone(p.phone)).join(', ')
		].join(' · ');
	});
	const confirmText = $derived(
		t.confirm(
			`${clients?.length ?? ids.length} ${plural(clients?.length ?? ids.length, t.clientsForms)}`
		)
	);

	function pick(f: MergeField, id: number) {
		if (choice) choice = { ...choice, from: { ...choice.from, [f]: id } };
	}

	async function merge() {
		if (!request || !clients || tooMany) return;
		try {
			const merged = await mergeClients(request);
			open = false;
			onmerged(merged);
		} catch (e) {
			if (e instanceof ApiError && e.status === 409) {
				stale = true;
				await load(true);
			} else {
				toast.show({ message: t.failed(errorMessage(e)), tone: 'danger' });
			}
		}
	}
</script>

<Dialog bind:open title={t.title} description={t.hint} size="lg">
	{#if clients && choice}
		{@const ch = choice}
		<div class="body">
			{#if stale}
				<Callout tone="danger">{t.stale}</Callout>
			{/if}
			<div class="scroll">
				<div class="grid" style:--cols={clients.length}>
					<span class="label"></span>
					{#each clients as c (c.id)}
						<label class="head" class:kept={ch.targetId === c.id}>
							<input
								type="radio"
								name="{uid}-target"
								aria-label={t.keepClient(c.id)}
								checked={ch.targetId === c.id}
								onchange={() => (choice = { ...ch, targetId: c.id })}
							/>
							<span class="num">{ru.client.number(c.id)}</span>
							<span class="keep">{t.keep}</span>
						</label>
					{/each}

					{#each FIELD_ROWS as row (row.field)}
						<span class="label">{row.label}</span>
						{#each clients as c (c.id)}
							<label class="cell" class:chosen={ch.from[row.field] === c.id}>
								<input
									type="radio"
									name="{uid}-{row.field}"
									aria-label={t.takeFrom(row.label, c.id)}
									checked={ch.from[row.field] === c.id}
									onchange={() => pick(row.field, c.id)}
								/>
								<span class="value" class:muted={shown(c, row.field) === t.empty}
									>{shown(c, row.field)}</span
								>
							</label>
						{/each}
					{/each}

					<span class="label">{ru.client.phones}</span>
					{#each clients as c (c.id)}
						<span class="cell static num">
							{#each phonesOf(c) as line, i (i)}<span>{line}</span>{/each}
						</span>
					{/each}
					<span class="label">{ru.orders.title}</span>
					{#each clients as c (c.id)}
						<span class="cell static num">{c.orders_count}</span>
					{/each}
				</div>
			</div>
			{#if joined.length > MAX_CLIENT_PHONES}
				<Callout tone="danger" title={t.phonesTooMany(joined.length, MAX_CLIENT_PHONES)}>
					<div role="group" aria-label={t.phonesGroup} class="keep-phones">
						{#each joined as p (phoneKey(p.phone))}
							<label class="keep-phone">
								<input
									type="checkbox"
									aria-label={t.phoneKeep(formatPhone(p.phone))}
									checked={!dropped.has(phoneKey(p.phone))}
									onchange={(e) => keepPhone(p.phone, e.currentTarget.checked)}
								/>
								<span class="num">{formatPhone(p.phone)}</span>
								{#if p.note}<span class="muted">{p.note}</span>{/if}
							</label>
						{/each}
					</div>
					<p class="left" aria-live="polite">{t.phonesLeft(kept.length, MAX_CLIENT_PHONES)}</p>
				</Callout>
			{/if}
			<p class="summary" aria-live="polite">{summary}</p>
		</div>
	{:else if loadError}
		<Callout tone="danger" title={t.loadFailed}>
			{loadError}
			{#snippet actions()}
				<Button size="sm" onclick={() => void load(false)}>{ru.common.retry}</Button>
			{/snippet}
		</Callout>
	{:else}
		<div class="loading" aria-busy="true">
			<Skeleton height="2.5rem" />
			<Skeleton height="6rem" />
		</div>
	{/if}

	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" disabled={!request || tooMany} onclick={() => (confirming = true)}
			>{t.action}</Button
		>
	{/snippet}
</Dialog>

<ConfirmDialog
	bind:open={confirming}
	title={t.action}
	description={confirmText}
	confirmLabel={t.confirmLabel}
	tone="primary"
	onconfirm={merge}
/>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.scroll {
		overflow-x: auto;
	}
	.keep-phones {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
	}
	.keep-phone {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
	}
	.left {
		margin: var(--space-1) 0 0;
	}
	.grid {
		display: grid;
		/* As wide as the columns need: a sticky label stays inside its grid while the table scrolls. */
		width: max-content;
		min-width: 100%;
		grid-template-columns: 6rem repeat(var(--cols), minmax(10rem, 1fr));
		gap: var(--space-1);
		align-items: stretch;
		font-size: var(--text-sm);
	}
	/* The labels stay in view while the clients scroll sideways. */
	.label {
		position: sticky;
		left: 0;
		z-index: 1;
		background: var(--surface);
		padding-top: var(--space-2);
		color: var(--text-3);
	}
	.head,
	.cell {
		display: flex;
		gap: var(--space-2);
		align-items: flex-start;
		box-sizing: border-box;
		padding: var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		cursor: pointer;
	}
	.head {
		flex-wrap: wrap;
		font-weight: 600;
	}
	.keep {
		flex-basis: 100%;
		color: var(--text-3);
		font-weight: 400;
	}
	.head.kept,
	.cell.chosen {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.cell.static {
		flex-direction: column;
		gap: 0;
		cursor: default;
	}
	.value {
		min-width: 0;
		overflow-wrap: anywhere;
		white-space: pre-wrap;
	}
	.muted {
		color: var(--text-3);
	}
	.summary {
		margin: 0;
		color: var(--text);
		font-weight: 500;
	}
	.loading {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
</style>
