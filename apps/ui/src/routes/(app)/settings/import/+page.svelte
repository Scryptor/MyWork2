<!-- «Импорт из MyWork 2» (spec 6.8): the past imports (date, folder, orders, warnings) lead to their reports;
     «Перенести снова» replaces the data of the program with a folder of MyWork 2, after a confirmation (a copy
     is made first). The replacement is for SQLite only and for a server on this computer only (the path is on
     its disk). An import under way shows its progress here. An import made before the stock module kept the
     stock as it was: while the base has no stock, «Перенести склад из прошлого импорта» transfers it from there
     (decision 17; any server, the folder is not needed). Once the stock is started by hand the button is gone and
     a notice says why and what is left. Rights: «settings_manage» and «users_manage». -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { ImportJob } from '$api/ImportJob';
	import type { LegacyImportItem } from '$api/LegacyImportItem';
	import type { LegacyStockState } from '$api/LegacyStockState';
	import { getAbout } from '#lib/api/catalogs.js';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import {
		currentLegacyImport,
		legacyStockState,
		listLegacyImports,
		migrateLegacyStock
	} from '#lib/api/import.js';
	import { app } from '#lib/app/context.svelte.js';
	import { formatFull } from '#lib/format/date.js';
	import { groupDigits } from '#lib/format/digits.js';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';
	import { reportRoute } from '#lib/import/report.js';
	import ImportWizard from '#lib/setup/ImportWizard.svelte';
	import { jobState } from '#lib/setup/progress.js';
	import { stockCount } from '#lib/stock/summary.svelte.js';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.legacy;
	const uid = $props.id();
	const allowed = app.can('settings_manage') && app.can('users_manage');

	let items = $state.raw<LegacyImportItem[] | null>(null);
	let failure = $state<string | null>(null);
	/** Can the data be replaced here: null until known. */
	let replaceable = $state<boolean | null>(null);
	/** Why not, when the server says it (a server on the network). */
	let notHere = $state<string | null>(null);
	/** The server did not say which base it has (no connection, say): not a reason to say «только SQLite». */
	let unknown = $state<string | null>(null);
	let wizard = $state(false);
	let resume = $state.raw<ImportJob | null>(null);
	/** The stock of the last import, kept as it was: offered while `available`. */
	let stock = $state.raw<LegacyStockState | null>(null);
	let migrating = $state(false);
	let stockFailure = $state<string | null>(null);

	onMount(async () => {
		if (!allowed) return;
		const [list, about, job, stockState] = await Promise.allSettled([
			listLegacyImports(),
			getAbout(),
			currentLegacyImport(),
			legacyStockState()
		]);
		// Without an answer about the stock there is nothing to offer: the rest of the page stands.
		if (stockState.status === 'fulfilled') stock = stockState.value;
		if (list.status === 'fulfilled') items = list.value;
		else failure = errorMessage(list.reason);
		if (job.status === 'rejected' && job.reason instanceof ApiError && job.reason.status === 403)
			notHere = job.reason.message;
		if (job.status === 'fulfilled' && job.value && jobState(job.value).kind === 'running') {
			resume = job.value;
			wizard = true;
		}
		if (about.status === 'rejected' && notHere === null) unknown = errorMessage(about.reason);
		replaceable =
			notHere === null && about.status === 'fulfilled' && about.value.database === 'SQLite';
	});

	/** One request at a time (the server refuses a repeat anyway); the report of the import shows the result. */
	async function migrateStock() {
		if (migrating) return;
		migrating = true;
		stockFailure = null;
		try {
			const done = await migrateLegacyStock();
			toast.show({
				message: t.stockDone(
					countLabel(done.stock_items, t.stockItemForms),
					countLabel(done.order_parts, t.stockPartForms)
				),
				tone: 'success'
			});
			stock = null;
			// Positions below their threshold go into «Докупить» of the sidebar at once.
			stockCount.refresh();
			await goto(resolve(reportRoute(done.import_id)));
		} catch (e) {
			stockFailure =
				(e instanceof ApiError ? e.body?.fields[0]?.message : undefined) ?? errorMessage(e);
			// A refusal can mean there is nothing to transfer any more: the offer follows the server.
			try {
				stock = await legacyStockState();
			} catch {
				// Without an answer the offer stays as it was; the message above explains the refusal.
			}
		} finally {
			migrating = false;
		}
	}
</script>

<h1 class="title">{ru.settings.import}</h1>

{#if !allowed}
	<Callout tone="info">{t.rights}</Callout>
{:else}
	{#if stock?.available && !wizard}
		<Callout tone="warning" title={t.stockTitle}>
			{t.stockText(
				countLabel(stock.stock_rows, t.stockItemForms),
				countLabel(stock.stock_map_rows, t.stockWriteOffForms)
			)}
			{#snippet actions()}
				<Button variant="primary" busy={migrating} onclick={migrateStock}>{t.stockMigrate}</Button>
			{/snippet}
		</Callout>
	{:else if stock?.blocked_by_stock && stock.import_id !== null && !wizard}
		{@const report = resolve(reportRoute(stock.import_id))}
		<Callout tone="info" title={t.stockBlockedTitle}>
			{t.stockBlockedText}
			{#snippet actions()}
				<Button size="sm" href={report}>{t.stockBlockedReport}</Button>
			{/snippet}
		</Callout>
	{/if}
	{#if stockFailure && !wizard}<Callout tone="danger">{stockFailure}</Callout>{/if}
	{#if wizard}
		<Card>
			<ImportWizard replace {resume} onback={resume ? undefined : () => (wizard = false)} />
		</Card>
	{:else if replaceable}
		<div class="again">
			<p class="hint">
				{t.againHint}
				<HelpHint title={t.replaceHelp} text={ru.help.import.replace} />
			</p>
			<Button onclick={() => (wizard = true)}>{items?.length ? t.again : t.first}</Button>
		</div>
	{:else if notHere}
		<Callout tone="info">{notHere}</Callout>
	{:else if unknown}
		<Callout tone="danger">{unknown}</Callout>
	{:else if replaceable === false}
		<p class="hint">{t.sqliteOnly}</p>
	{/if}

	<section class="past" aria-labelledby="{uid}-past">
		<h2 id="{uid}-past">{t.past}</h2>
		{#if failure}
			<Callout tone="danger">{failure}</Callout>
		{:else if items === null}
			<Skeleton height="3.5rem" radius="md" />
		{:else if items.length === 0}
			<p class="hint">{t.none}</p>
		{:else}
			<ul class="items" aria-label={t.past}>
				{#each items as item (item.id)}
					<li>
						<a class="item" href={resolve(reportRoute(item.id))}>
							<span class="when">
								<span>{formatFull(item.imported_at)}</span>
								{#if item.mode === 'replace'}<Badge tone="warning">{t.replaced}</Badge>{/if}
							</span>
							<span class="path">{item.source_path}</span>
							<span class="meta">
								{t.itemOrders(groupDigits(String(item.orders)))} · {t.itemWarnings(
									groupDigits(String(item.warnings))
								)}
							</span>
						</a>
					</li>
				{/each}
			</ul>
		{/if}
	</section>
{/if}

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.again {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
	}
	.hint {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-md);
	}
	.again .hint {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}
	.past {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	h2 {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.items {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.item {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		font-size: var(--text-md);
		text-decoration: none;
	}
	.item:hover {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.when {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		font-weight: 600;
	}
	.path {
		min-width: 0;
		overflow-wrap: anywhere;
		color: var(--text-2);
	}
	.meta {
		color: var(--text-3);
	}
</style>
