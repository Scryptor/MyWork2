<!-- The report of a past import (spec 6.9): a number of a group opens the order's panel over it, ‹ › go along
     the numbers of that group. -->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { ImportReportView } from '$api/ImportReportView';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { getLegacyImport } from '#lib/api/import.js';
	import { ru } from '#lib/i18n/ru.js';
	import ReportView from '#lib/import/ReportView.svelte';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';

	const t = ru.legacy;
	const id = $derived(Number(page.params.id));
	let report = $state.raw<ImportReportView | null>(null);
	let failure = $state<string | null>(null);

	$effect(() => {
		const wanted = id;
		report = null;
		failure = null;
		if (!Number.isInteger(wanted) || wanted <= 0) {
			failure = t.notFound;
			return;
		}
		let gone = false;
		getLegacyImport(wanted).then(
			(r) => {
				if (!gone) report = r;
			},
			(e: unknown) => {
				if (gone) return;
				// Without the rights (a user of MyWork 2 that may not import): the server's words.
				failure = e instanceof ApiError && e.status === 404 ? t.notFound : errorMessage(e);
			}
		);
		return () => {
			gone = true;
		};
	});
</script>

{#if report}
	<ReportView
		{report}
		onorder={(order, ids) => void orderPanel.open(order, { neighbors: () => [...ids] })}
	/>
{:else if failure}
	<h1 class="title">{t.report}</h1>
	<Callout tone="danger">{failure}</Callout>
	<div><Button href={resolve('settings/import')}>{t.past}</Button></div>
{:else}
	<h1 class="title">{t.report}</h1>
	<Skeleton height="6rem" radius="md" />
{/if}

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
</style>
