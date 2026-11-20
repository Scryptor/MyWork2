<!-- «Статусы» (spec 6.7): the statuses of an order — drag order, colour, name in place, kind with a warning
     before an order's kind is changed, how many orders are in each, archive, a new status. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { onMount, untrack, type Snippet } from 'svelte';
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
	import { display } from '#lib/design/prefs.svelte.js';
	import { countLabel as countOf } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import CatalogRows from '#lib/settings/CatalogRows.svelte';
	import { createNamedCatalog } from '#lib/settings/named-catalog.svelte.js';
	import StatusColorButton from '#lib/settings/StatusColorButton.svelte';
	import {
		KIND_HELP,
		KIND_ICONS,
		startKindChange,
		STATUS_KINDS
	} from '#lib/settings/status-kind.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
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

<div class="heading">
	<h1 class="title">{ru.settings.statuses}</h1>
	<HelpHint title={ru.settings.statuses} text={ru.help.statuses} />
</div>

{#if readonly}<Callout tone="info">{ru.settings.readOnly}</Callout>{/if}

<!-- The kind: its icon, then the list. -->
{#snippet kindCell(k: StatusKind, select: Snippet)}
	{@const Icon = KIND_ICONS[k]}
	<span class="kind" style:grid-area="kind">
		<Icon size={16} aria-hidden="true" class="kind-icon" />
		<span class="select">{@render select()}</span>
	</span>
{/snippet}

<!-- The form of a new status, in the columns of the rows: name, colour, kind, «Добавить». -->
{#snippet addRow()}
	<form
		class="add"
		onsubmit={(e) => {
			e.preventDefault();
			void add();
		}}
	>
		<div class="cell" style:grid-area="name">
			<TextInput
				ariaLabel={t.add}
				placeholder={t.addPlaceholder}
				autocomplete="off"
				error={nameError}
				bind:value={() => name, (v) => ((name = v), (nameError = undefined))}
			/>
		</div>
		<span class="cell" style:grid-area="color">
			<StatusColorButton {color} name={name.trim() || t.addColor} onchange={(h) => (color = h)} />
		</span>
		{#snippet addKind()}
			<Select
				ariaLabel={t.addKind}
				items={kindItems}
				bind:value={() => kind, (v) => v && (kind = v)}
			/>
		{/snippet}
		<span class="cell" style:grid-area="kind">{@render kindCell(kind, addKind)}</span>
		<!-- Where «В архив» is: at the start of the column, at its end when a row takes two lines (narrow). -->
		<span class="cell" class:end={display.narrow} style:grid-area="actions">
			<!-- Words alone fit the column of «В архив»; on a phone, where they do not, the icon alone. -->
			{#if display.phone}
				<Button type="submit" size="sm" icon={Plus} busy={adding} aria-label={t.addButton} />
			{:else}
				<Button type="submit" size="sm" busy={adding} aria-label={t.addButton}>{t.addShort}</Button>
			{/if}
		</span>
	</form>
{/snippet}

<Card>
	<div class="body">
		<p class="note">{t.hint}</p>

		<CatalogRows
			rows={catalog.loadError ? [] : catalog.rows}
			{countLabel}
			{readonly}
			columns={[
				{ id: 'color', title: t.color, width: 'var(--status-color-width)' },
				{ id: 'kind', title: t.kind, help: KIND_HELP, width: 'var(--status-kind-width)' }
			]}
			add={readonly ? undefined : addRow}
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
					<span class="color" style:grid-area="color">
						<StatusColorButton
							color={s.color}
							name={s.name}
							disabled={readonly}
							onchange={(h) => void patch(s.id, { color: h })}
						/>
					</span>
					{#key redraw[s.id] ?? 0}
						{#snippet rowKind()}
							<Select
								ariaLabel={t.kindOf(s.name)}
								items={kindItems}
								disabled={readonly}
								bind:value={() => s.kind, (v) => pickKind(s, v)}
							/>
						{/snippet}
						{@render kindCell(s.kind, rowKind)}
					{/key}
				{/if}
			{/snippet}
		</CatalogRows>

		{#if catalog.loadError}
			<Callout tone="danger">{t.loadFailed}: {catalog.loadError}</Callout>
		{:else if catalog.loading && catalog.rows.length === 0}
			<Skeleton height="2rem" />
		{:else if nothing}
			<EmptyState title={t.empty} />
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
	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.body {
		/* The colour and the kind columns: the round button (its touch size too) and the icon with the list. */
		--status-color-width: var(--control-md);
		--status-kind-width: 10.25rem;
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
	/* The cells of the form are those of the grid of the rows. */
	.add {
		display: contents;
	}
	.cell {
		display: flex;
		align-items: center;
		min-width: 0;
		min-height: var(--control-md);
	}
	.cell > :global(*) {
		flex: 1;
		min-width: 0;
	}
	.cell > :global(button) {
		flex: none;
	}
	.cell.end {
		justify-content: flex-end;
	}
	.color {
		display: flex;
		align-items: center;
	}
	.kind {
		display: flex;
		align-items: center;
		gap: var(--space-1-5);
		min-width: 0;
		max-width: var(--status-kind-width);
	}
	.kind :global(.kind-icon) {
		flex: none;
		color: var(--text-3);
	}
	.select {
		flex: 1;
		min-width: 0;
	}
</style>
