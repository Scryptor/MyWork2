<!-- «Импорт из MyWork 2» (spec 6.8): the past imports (date, folder, orders, warnings) lead to their reports;
     «Перенести снова» replaces the data of the program with a folder of MyWork 2, after a confirmation (a copy
     is made first). The replacement is for SQLite only and for a server on this computer only (the path is on
     its disk). An import under way shows its progress here. Rights: «settings_manage» and «users_manage». -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import type { ImportJob } from '$api/ImportJob';
	import type { LegacyImportItem } from '$api/LegacyImportItem';
	import { getAbout } from '#lib/api/catalogs.js';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { currentLegacyImport, listLegacyImports } from '#lib/api/import.js';
	import { app } from '#lib/app/context.svelte.js';
	import { formatFull } from '#lib/format/date.js';
	import { groupDigits } from '#lib/format/digits.js';
	import { ru } from '#lib/i18n/ru.js';
	import { reportRoute } from '#lib/import/report.js';
	import ImportWizard from '#lib/setup/ImportWizard.svelte';
	import { jobState } from '#lib/setup/progress.js';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';

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

	onMount(async () => {
		if (!allowed) return;
		const [list, about, job] = await Promise.allSettled([
			listLegacyImports(),
			getAbout(),
			currentLegacyImport()
		]);
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
</script>

<h1 class="title">{ru.settings.import}</h1>

{#if !allowed}
	<Callout tone="info">{t.rights}</Callout>
{:else}
	{#if wizard}
		<Card>
			<ImportWizard replace {resume} onback={resume ? undefined : () => (wizard = false)} />
		</Card>
	{:else if replaceable}
		<div class="again">
			<p class="hint">{t.againHint}</p>
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
