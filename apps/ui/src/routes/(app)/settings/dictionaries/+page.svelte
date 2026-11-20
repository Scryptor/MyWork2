<!-- «Справочники» (spec 6.8): the ten dictionaries of the intake form. The kind is in the address (`?kind=`);
     the values are searched, added, dragged, renamed in place, archived, deleted and merged. -->
<script lang="ts">
	import Merge from '@lucide/svelte/icons/merge';
	import Search from '@lucide/svelte/icons/search';
	import { untrack } from 'svelte';
	import type { DictionaryKind } from '$api/DictionaryKind';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { app } from '#lib/app/context.svelte.js';
	import { display } from '#lib/design/prefs.svelte.js';
	import { countLabel as countOf } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import { createCatalog } from '#lib/settings/catalog.svelte.js';
	import CatalogRows from '#lib/settings/CatalogRows.svelte';
	import MergeDialog from '#lib/settings/MergeDialog.svelte';
	import RenameEverywhereDialog from '#lib/settings/RenameEverywhereDialog.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Select from '#lib/ui/Select.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.dictionaries;
	const KINDS = Object.keys(t.kindNames) as DictionaryKind[];
	const catalog = createCatalog(undefined, () => app.can('settings_manage'));

	const kind = $derived.by(() => {
		const raw = page.url.searchParams.get('kind');
		return KINDS.find((k) => k === raw) ?? KINDS[0]!;
	});
	/** The source of a client counts clients, every other dictionary counts orders. */
	const ofClients = $derived(kind === 'client_source');
	const countLabel = (n: number) =>
		ofClients ? t.countAt(countOf(n, t.atClients)) : t.countIn(countOf(n, t.inOrders));
	const readonly = $derived(!app.can('settings_manage'));

	let selected = $state<number[]>([]);
	let newValue = $state('');
	let newError = $state<string | undefined>();
	let removing = $state<number | null>(null);
	let removeOpen = $state(false);
	/** The rename that waits for the choice «и в заказах»; the window is open while it is set. `where`: «в 3 заказах». */
	let renaming = $state<{ id: number; from: string; to: string; where: string } | null>(null);
	let mergeOpen = $state(false);

	$effect(() => {
		const k = kind;
		untrack(() => {
			selected = [];
			newValue = '';
			newError = undefined;
			void catalog.load(k);
		});
	});

	const rows = $derived([...catalog.active, ...catalog.archived]);
	const picked = $derived(catalog.all.filter((r) => selected.includes(r.id) && !r.archived));
	const nothing = $derived(!catalog.loading && !catalog.loadError && rows.length === 0);
	const where = $derived(ofClients ? t.atClientsWhere : t.inOrdersWhere);
	const changedForms = $derived(ofClients ? t.changedClients : t.changedOrders);
	/** «Мастер» and «Гарантия» are also the defaults of a new order: those follow a rename and a merge. */
	const followsDefaults = $derived(kind === 'master' || kind === 'warranty');

	/** The program's copy of the settings, after the server moved a default along with its value. */
	function defaultsMoved() {
		if (followsDefaults) void app.refreshCatalogs().catch(() => {});
	}

	function pickKind(k: DictionaryKind | null) {
		if (!k || k === kind) return;
		const search = new URLSearchParams({ kind: k });
		void goto(resolve(`settings/dictionaries?${search}`), { replace: true, reset: false });
	}

	const fail = (message: string) => toast.show({ message, tone: 'danger' });

	async function add() {
		const value = newValue.trim();
		if (readonly) return;
		if (!value) {
			newError = ru.settings.required;
			return;
		}
		const error = await catalog.add(value);
		if (error) newError = error;
		else {
			newValue = '';
			newError = undefined;
		}
	}

	async function rename(id: number, label: string) {
		const row = catalog.all.find((r) => r.id === id);
		if (!row) return;
		// Not known how many records have it: ask, as for a value in use.
		if (row.count === null || row.count > 0) {
			const at = row.count === null ? where : countLabel(row.count);
			renaming = { id, from: row.label, to: label, where: at };
			return;
		}
		const error = await catalog.rename(id, label);
		if (error) fail(error);
		else defaultsMoved();
	}

	async function renameEverywhere(inOrders: boolean): Promise<string | null> {
		if (!renaming) return null;
		const r = await catalog.renameEverywhere(renaming.id, renaming.to, inOrders);
		if (r.error) return r.error;
		defaultsMoved();
		toast.show({
			message: inOrders ? t.renamed(countOf(r.changed, changedForms)) : t.renamedPlain,
			tone: 'success'
		});
		return null;
	}

	/** The value is renamed to itself: the orders whose spelling differs get the one of the dictionary. */
	async function align(id: number) {
		const row = catalog.all.find((r) => r.id === id);
		if (!row) return;
		const r = await catalog.renameEverywhere(id, row.label, true);
		if (r.error) fail(r.error);
		else toast.show({ message: t.aligned(countOf(r.changed, changedForms)), tone: 'success' });
	}

	async function merge(targetId: number, inOrders: boolean): Promise<string | null> {
		// What the dialog showed: a value archived meanwhile (elsewhere) is not merged unseen.
		const r = await catalog.merge(
			targetId,
			picked.map((p) => p.id).filter((id) => id !== targetId),
			inOrders
		);
		if (r.error) return r.error;
		defaultsMoved();
		selected = [];
		toast.show({
			message: inOrders ? t.merged(countOf(r.changed, changedForms)) : t.mergedPlain,
			tone: 'success'
		});
		return null;
	}

	async function archive(id: number, archived: boolean) {
		const error = await catalog.setArchived(id, archived);
		if (error) fail(error);
		else if (archived) selected = selected.filter((x) => x !== id);
	}

	async function remove() {
		if (removing === null) return;
		const error = await catalog.remove(removing);
		if (error) fail(error);
		else {
			selected = selected.filter((x) => x !== removing);
			toast.show({ message: t.removed });
		}
	}

	async function reorder(ids: number[]) {
		const error = await catalog.reorder(ids);
		if (error) fail(error);
	}

	const removingLabel = $derived(catalog.all.find((r) => r.id === removing)?.label ?? '');
</script>

<div class="heading">
	<h1 class="title">{ru.settings.dictionaries}</h1>
	<HelpHint title={ru.settings.dictionaries} text={ru.help.dictionaries} />
</div>

{#if readonly}<Callout tone="info">{ru.settings.readOnly}</Callout>{/if}

<div class="screen" class:narrow={display.narrow}>
	{#if display.narrow}
		<Select
			label={t.kind}
			bind:value={() => kind, (v) => pickKind(v)}
			items={KINDS.map((k) => ({ value: k, label: t.kindNames[k] }))}
		/>
	{:else}
		<nav class="kinds" aria-label={t.kinds}>
			{#each KINDS as k (k)}
				<button
					type="button"
					class="kind"
					class:current={k === kind}
					aria-current={k === kind ? 'page' : undefined}
					onclick={() => pickKind(k)}
				>
					{t.kindNames[k]}
				</button>
			{/each}
		</nav>
	{/if}

	<Card>
		<div class="body">
			<div class="tools">
				<TextInput
					ariaLabel={t.search}
					placeholder={t.search}
					autocomplete="off"
					bind:value={catalog.query}
				>
					{#snippet prefix()}<Search size={16} aria-hidden="true" />{/snippet}
				</TextInput>
				{#if !readonly}
					<form
						onsubmit={(e) => {
							e.preventDefault();
							void add();
						}}
					>
						<TextInput
							ariaLabel={t.add}
							placeholder={t.add}
							autocomplete="off"
							error={newError}
							bind:value={() => newValue, (v) => ((newValue = v), (newError = undefined))}
						/>
					</form>
				{/if}
			</div>

			{#if selected.length >= 2 && !readonly}
				<div class="bar">
					<Button icon={Merge} onclick={() => (mergeOpen = true)}>{t.mergeSelected}</Button>
				</div>
			{/if}

			{#if catalog.loadError}
				<Callout tone="danger">{t.loadFailed}: {catalog.loadError}</Callout>
			{:else if catalog.loading && rows.length === 0}
				<Skeleton height="2rem" />
			{:else if nothing}
				<EmptyState title={catalog.query.trim() ? t.nothingFound : t.empty} />
			{:else}
				<CatalogRows
					{rows}
					{countLabel}
					selectable
					bind:selected
					{readonly}
					onrename={(id, label) => void rename(id, label)}
					onarchive={(id, a) => void archive(id, a)}
					onalign={(id) => void align(id)}
					ondelete={(id) => {
						removing = id;
						removeOpen = true;
					}}
					onreorder={(ids) => void reorder(ids)}
				/>
			{/if}
		</div>
	</Card>
</div>

<ConfirmDialog
	bind:open={removeOpen}
	title={t.removeTitle(removingLabel)}
	description={t.removeText}
	confirmLabel={t.remove}
	onconfirm={remove}
/>

{#if renaming}
	<RenameEverywhereDialog
		bind:open={() => renaming !== null, (v) => !v && (renaming = null)}
		from={renaming.from}
		to={renaming.to}
		where={renaming.where}
		warranty={kind === 'warranty'}
		onconfirm={renameEverywhere}
	/>
{/if}

<MergeDialog
	bind:open={mergeOpen}
	rows={picked}
	{countLabel}
	{where}
	warranty={kind === 'warranty'}
	onconfirm={merge}
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
	.screen {
		display: grid;
		grid-template-columns: 11rem minmax(0, 1fr);
		align-items: start;
		gap: var(--space-3);
	}
	.screen.narrow {
		grid-template-columns: minmax(0, 1fr);
	}
	.kinds {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
	}
	.kind {
		min-height: var(--control-md);
		padding: 0 var(--space-2);
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text-2);
		font: inherit;
		font-size: var(--text-md);
		text-align: start;
		cursor: pointer;
	}
	.kind:hover {
		background: var(--surface-3);
		color: var(--text);
	}
	.kind.current {
		background: var(--accent-soft);
		color: var(--accent-text);
		font-weight: 600;
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-3);
	}
	.tools {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.bar {
		display: flex;
	}
</style>
