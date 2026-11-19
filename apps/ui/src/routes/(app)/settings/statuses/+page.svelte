<!-- «Статусы» (spec 6.7): the statuses of an order — drag order, colour, name in place, kind with a warning
     before an order's kind is changed, how many orders are in each, archive, a new status. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { onMount, untrack } from 'svelte';
	import type { NewStatus } from '$api/NewStatus';
	import type { Status } from '$api/Status';
	import type { StatusKind } from '$api/StatusKind';
	import type { StatusPatch } from '$api/StatusPatch';
	import {
		createStatus,
		listStatuses,
		reorderStatuses,
		statusUsage,
		updateStatus
	} from '#lib/api/catalogs.js';
	import { app } from '#lib/app/context.svelte.js';
	import { countLabel as countOf } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import CatalogRows from '#lib/settings/CatalogRows.svelte';
	import { createNamedCatalog } from '#lib/settings/named-catalog.svelte.js';
	import StatusColorButton from '#lib/settings/StatusColorButton.svelte';
	import { KIND_HINTS, startKindChange, STATUS_KINDS } from '#lib/settings/status-kind.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.statuses;
	const catalog = createNamedCatalog<Status, NewStatus, StatusPatch>(
		{
			list: () => listStatuses(true),
			usage: statusUsage,
			add: createStatus,
			update: updateStatus,
			reorder: reorderStatuses
		},
		() => app.refreshCatalogs(),
		() => app.can('settings_manage')
	);
	const readonly = $derived(!app.can('settings_manage'));
	const countLabel = (n: number) => ru.dictionaries.countIn(countOf(n, ru.dictionaries.inOrders));
	const kindItems = STATUS_KINDS.map((k) => ({ value: k, label: t.kindNames[k] }));

	onMount(() => void catalog.load());

	const fail = (message: string) => toast.show({ message, tone: 'danger' });
	const byId = (id: number) => catalog.items.find((s) => s.id === id);

	/**
	 * A select that kept its old kind after a cancel or a refusal is drawn again from the data — that one only:
	 * the other rows keep their selects (and the focus in them).
	 */
	let redraw = $state<Record<number, number>>({});
	const redrawRow = (id: number) => (redraw = { ...redraw, [id]: (redraw[id] ?? 0) + 1 });

	async function patch(id: number, p: StatusPatch) {
		const error = await catalog.update(id, p);
		if (error) {
			fail(error);
			redrawRow(id);
		}
	}

	let changing = $state<ReturnType<typeof startKindChange> | null>(null);
	let changeOpen = $state(false);

	$effect(() => {
		if (changeOpen) return;
		untrack(() => {
			if (!changing) return;
			// Cancelled (a confirmed change has the new kind already: drawing it again changes nothing).
			redrawRow(changing.id);
		});
	});

	function pickKind(s: Status, to: StatusKind | null) {
		if (!to || to === s.kind) return;
		const next = startKindChange(s, to, catalog.countOf(s.id));
		if (next.warning === null) {
			void patch(s.id, { kind: to });
			return;
		}
		changing = next;
		changeOpen = true;
	}

	async function confirmKind() {
		if (!changing) return;
		await patch(changing.id, { kind: changing.to });
	}

	let name = $state('');
	let kind = $state<StatusKind>('open');
	let color = $state('gray');
	let nameError = $state<string | undefined>();
	let adding = $state(false);

	async function add() {
		const value = name.trim();
		if (readonly || adding) return;
		if (!value) {
			nameError = ru.settings.required;
			return;
		}
		adding = true;
		const error = await catalog.add({ name: value, kind, color });
		adding = false;
		if (error) nameError = error;
		else {
			name = '';
			kind = 'open';
			color = 'gray';
			nameError = undefined;
		}
	}

	const nothing = $derived(!catalog.loading && !catalog.loadError && catalog.rows.length === 0);
</script>

<h1 class="title">{ru.settings.statuses}</h1>

{#if readonly}<Callout tone="info">{ru.settings.readOnly}</Callout>{/if}

<Card>
	<div class="body">
		<p class="note">{t.hint}</p>

		{#if !readonly}
			<form
				class="add"
				onsubmit={(e) => {
					e.preventDefault();
					void add();
				}}
			>
				<div class="grow">
					<TextInput
						ariaLabel={t.add}
						placeholder={t.add}
						autocomplete="off"
						error={nameError}
						bind:value={() => name, (v) => ((name = v), (nameError = undefined))}
					/>
				</div>
				<div class="kind">
					<Select
						label={t.addKind}
						items={kindItems}
						bind:value={() => kind, (v) => v && (kind = v)}
						hint={KIND_HINTS[kind]}
					/>
				</div>
				<StatusColorButton {color} name={name.trim() || t.addColor} onchange={(h) => (color = h)} />
				<Button type="submit" icon={Plus} busy={adding}>{t.addButton}</Button>
			</form>
		{/if}

		{#if catalog.loadError}
			<Callout tone="danger">{t.loadFailed}: {catalog.loadError}</Callout>
		{:else if catalog.loading && catalog.rows.length === 0}
			<Skeleton height="2rem" />
		{:else if nothing}
			<EmptyState title={t.empty} />
		{:else}
			<CatalogRows
				rows={catalog.rows}
				{countLabel}
				{readonly}
				onrename={(id, label) => void patch(id, { name: label })}
				onarchive={(id, archived) => void patch(id, { archived })}
				onreorder={async (ids) => {
					const error = await catalog.reorder(ids);
					if (error) fail(error);
				}}
			>
				{#snippet extra(id)}
					{@const s = byId(id)}
					{#if s}
						<span class="more">
							<StatusColorButton
								color={s.color}
								name={s.name}
								disabled={readonly}
								onchange={(h) => void patch(s.id, { color: h })}
							/>
							{#key redraw[s.id] ?? 0}
								<span class="kind">
									<Select
										label={t.kind}
										ariaLabel={t.kindOf(s.name)}
										items={kindItems}
										disabled={readonly}
										bind:value={() => s.kind, (v) => pickKind(s, v)}
										hint={KIND_HINTS[s.kind]}
									/>
								</span>
							{/key}
						</span>
					{/if}
				{/snippet}
			</CatalogRows>
		{/if}
	</div>
</Card>

<ConfirmDialog
	bind:open={changeOpen}
	title={t.changeTitle(changing?.name ?? '')}
	description={changing?.warning ?? ''}
	confirmLabel={t.changeConfirm}
	tone="primary"
	onconfirm={confirmKind}
/>

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-3);
	}
	.note {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-3);
	}
	.add {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		gap: var(--space-2);
	}
	.grow {
		flex: 1 1 14rem;
		min-width: 0;
	}
	.add .kind {
		flex: 1 1 14rem;
		min-width: 0;
	}
	.more {
		display: flex;
		flex: 1 1 18rem;
		align-items: flex-start;
		gap: var(--space-2);
		min-width: 0;
	}
	.more .kind {
		flex: 1;
		min-width: 0;
	}
</style>
