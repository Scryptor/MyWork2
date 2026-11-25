<!-- The panel of a stock position (decision 18): the fields saved by themselves, up to three photos, the
     history and «Удалить позицию». Without the right to edit it only reads. Which position is open lives
     in the address of «Склад». -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { untrack } from 'svelte';
	import type { StockItem } from '$api/StockItem';
	import type { StockRack } from '$api/StockRack';
	import { listDictionary } from '#lib/api/catalogs.js';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { deleteStockItem, getStockItem, listStockRacks } from '#lib/api/stock.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { leaveGuard } from '#lib/navigation/leave-guard.svelte.js';
	import OrderPhotos from '#lib/orders/OrderPhotos.svelte';
	import { createStockMedia, type Media } from '#lib/orders/photos.svelte.js';
	import SaveIndicator from '#lib/orders/SaveIndicator.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import SlideOver from '#lib/ui/SlideOver.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import {
		closeStockEditor,
		createStockEditor,
		stockUnsavedText,
		type StockEditor
	} from './editor.svelte.js';
	import { MAX_STOCK_PHOTOS, stockFieldErrors } from './fields.js';
	import StockFields from './StockFields.svelte';
	import StockPlaces from './StockPlaces.svelte';
	import StockHistory from './StockHistory.svelte';
	import { stockCount } from './summary.svelte.js';
	import { openStockItemId } from './view.js';

	let {
		itemId,
		onclose,
		onchanged,
		ondeleted
	}: {
		/** The position open (from the address); null: the panel is closed. */
		itemId: number | null;
		/** The panel asks to be closed; its edits are saved by then. */
		onclose: () => void;
		/** Every answer of the server about the position: the list catches up. */
		onchanged?: (item: StockItem) => void;
		/** The position was deleted: the list asks again. */
		ondeleted?: () => void;
	} = $props();

	const t = ru.stock;

	/** The position shown, one editor per position; another position gets a new one. */
	let editor = $state.raw<StockEditor | null>(null);
	let media = $state.raw<Media | null>(null);
	let loadError = $state<string | null>(null);
	/** The position is not there: deleted. Asking again cannot help. */
	let gone = $state(false);
	let dictionaries = $state.raw({
		part_type: [] as string[],
		device_type: [] as string[],
		brand: [] as string[],
		color: [] as string[]
	});
	let confirming = $state(false);
	/** The racks to put into; null until they are known (or when they could not be asked). */
	let racks = $state.raw<StockRack[] | null>(null);

	const item = $derived(editor?.item ?? null);
	const canEdit = $derived(app.can('stock_edit'));
	const canDelete = $derived(app.can('stock_delete'));
	const errors = $derived(stockFieldErrors(editor?.autosave.error ?? null));
	const title = $derived(item ? item.display_name : t.number(itemId ?? 0));

	// The choices come from the dictionaries, asked again at every opening: settings may have changed.
	const KINDS = ['part_type', 'device_type', 'brand', 'color'] as const;
	const isOpen = $derived(itemId !== null);
	$effect(() => {
		if (!isOpen) return;
		let current = true;
		void Promise.all(KINDS.map((k) => listDictionary(k).catch(() => []))).then((lists) => {
			// Without them the fields still take any text.
			if (!current) return;
			dictionaries = Object.fromEntries(
				KINDS.map((k, i) => [k, (lists[i] ?? []).map((d) => d.value)])
			) as typeof dictionaries;
		});
		return () => {
			current = false;
		};
	});

	// The racks are asked again at every opening: they may have been changed on the map.
	$effect(() => {
		if (!isOpen) return;
		let current = true;
		void listStockRacks({})
			.then((list) => {
				if (current) racks = list;
			})
			.catch(() => {
				if (current) racks = null;
			});
		return () => {
			current = false;
		};
	});

	let token = 0;
	$effect(() => {
		const id = itemId;
		untrack(() => void load(id));
	});

	async function load(id: number | null) {
		const mine = ++token;
		if (id === null) {
			swap(null);
			loadError = null;
			gone = false;
			return;
		}
		if (editor?.item.id === id) return;
		loadError = null;
		gone = false;
		// The position before is not shown while the next one loads.
		swap(null);
		try {
			const loaded = await getStockItem(id);
			if (mine !== token) return;
			swap(
				createStockEditor(loaded, {
					emit: (saved) => {
						onchanged?.(saved);
						// A quantity or a threshold may have moved the position in or out of «Докупить».
						stockCount.refresh();
					}
				})
			);
		} catch (e) {
			if (mine !== token) return;
			swap(null);
			gone = e instanceof ApiError && e.code === 'not_found';
			loadError = errorMessage(e);
		}
	}

	function swap(next: StockEditor | null) {
		const old = editor;
		editor = next;
		// What was typed is still saved; a failure is told.
		if (old) closeStockEditor(old);
		media?.detach();
		media?.retire();
		media = next ? createStockMedia(next.item.id) : null;
	}

	/** Own edits saved; a failure is told and the panel stays (its indicator offers «Повторить»). */
	async function saved(ed: StockEditor): Promise<boolean> {
		if (!ed.unsaved) return true;
		const ok = await ed.flush();
		if (!ok && editor === ed) toast.show({ message: stockUnsavedText(ed), tone: 'danger' });
		return ok;
	}

	/** Before another position is shown or the panel closes: own edits are saved; false keeps the panel. */
	function leave(): Promise<boolean> {
		return editor ? saved(editor) : Promise.resolve(true);
	}

	async function close() {
		if (await leave()) onclose();
	}

	// Any way out of this position waits for the save too and then goes on, as the client panel does. The
	// list's own changes (filter, sort) keep this position open: nothing to wait.
	leaveGuard<StockEditor>({
		editor: () => editor,
		unsaved: (ed) => ed.unsaved,
		saved,
		skip: (nav, ed) => {
			const to = nav.to?.url;
			return (
				!!to &&
				!nav.willUnload &&
				to.pathname === nav.from?.url.pathname &&
				openStockItemId(to.searchParams) === ed.item.id
			);
		}
	});
	// The screen goes: what is typed is still sent, a failure is told.
	$effect(
		() => () =>
			untrack(() => {
				if (editor) closeStockEditor(editor);
				media?.detach();
				media?.retire();
			})
	);

	/** «Удалить позицию»: on the newest version; what was typed goes first. */
	async function remove() {
		const ed = editor;
		if (!ed) return;
		try {
			await ed.run(async (version) => {
				await deleteStockItem(ed.item.id, version);
				return ed.item;
			});
		} catch (e) {
			// A refusal is told; the dialog stays for another try.
			toast.show({ message: t.deleteFailed(errorMessage(e)), tone: 'danger' });
			throw e;
		}
		stockCount.refresh();
		toast.show({ message: t.deleted });
		// Nothing of it is left to save.
		ed.drop();
		ondeleted?.();
		onclose();
	}

	const menu = $derived<MenuItem[]>([
		{ label: t.deleteItem, icon: Trash2, danger: true, onselect: () => (confirming = true) }
	]);
</script>

<SlideOver bind:open={() => itemId !== null, (v) => !v && void close()} {title}>
	{#snippet actions()}
		{#if item && canDelete}
			<DropdownMenu items={menu} label={ru.order.moreActions}>
				{#snippet trigger(props)}
					<IconButton {...props} label={ru.order.moreActions} icon={Ellipsis} variant="ghost" />
				{/snippet}
			</DropdownMenu>
		{/if}
	{/snippet}
	{#snippet header()}
		{#if item && editor}
			<p class="meta num">
				<span>{t.article(item.id)}</span>
				<SaveIndicator {editor} />
			</p>
		{/if}
	{/snippet}

	<div class="content">
		{#if editor && item}
			{@const ed = editor}
			{#key ed}
				<StockFields
					{item}
					autosave={ed.autosave}
					{dictionaries}
					readonly={!canEdit}
					{errors}
					conflicts={ed.conflicts}
					flashed={ed.flashed}
					onkeep={(f) => ed.keep(f)}
					ontake={(f) => ed.take(f)}
					onflashend={(f) => ed.endFlash(f)}
				/>
				<StockPlaces editor={ed} {racks} {canEdit} />
				{#if media}
					<Section title={ru.media.photoSection}>
						<OrderPhotos {media} max={MAX_STOCK_PHOTOS} readonly={!canEdit} />
					</Section>
				{/if}
				<StockHistory itemId={item.id} version={item.version} />
			{/key}
		{:else if gone}
			<Callout tone="info" title={t.gone}>{t.goneHint}</Callout>
		{:else if loadError}
			<Callout tone="danger" title={t.loadItemFailed}>
				{loadError}
				{#snippet actions()}
					<Button size="sm" onclick={() => void load(itemId)}>{ru.common.retry}</Button>
				{/snippet}
			</Callout>
		{:else}
			<div class="loading" aria-busy="true">
				<Skeleton height="2.5rem" />
				<Skeleton height="4rem" />
				<Skeleton height="2.5rem" width="80%" />
			</div>
		{/if}
	</div>
</SlideOver>

<ConfirmDialog
	bind:open={confirming}
	title={t.deleteTitle}
	description={t.deleteText}
	confirmLabel={t.deleteItem}
	onconfirm={remove}
/>

<style>
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1-5);
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.loading {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
</style>
