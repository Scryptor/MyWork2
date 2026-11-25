<!-- «Отчёты»: the report of the issued orders and, on the next tab, the transfers of orders between the
     points. The view (tab, period, filters, grouping) lives in the address: view.ts reads and writes it;
     every change asks the server again for the tab shown and calls the earlier request off. -->
<script lang="ts">
	import ChartColumn from '@lucide/svelte/icons/chart-column';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { MovesReport } from '$api/MovesReport';
	import type { OrderReport } from '$api/OrderReport';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { downloadOrderReport, movesReport, orderReport } from '#lib/api/reports.js';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { app } from '#lib/app/context.svelte.js';
	import { isoDay } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import MovesReportView from '#lib/reports/MovesReport.svelte';
	import OrdersReport from '#lib/reports/OrdersReport.svelte';
	import ReportFilters from '#lib/reports/ReportFilters.svelte';
	import {
		movesQuery,
		reportQuery,
		reportViewFromSearch,
		reportViewToSearch,
		type ReportView
	} from '#lib/reports/view.js';
	import Callout from '#lib/ui/Callout.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import { display } from '#lib/design/prefs.svelte.js';

	const t = ru.reports;

	const canView = $derived(app.can('reports_view'));
	const view = $derived(reportViewFromSearch(page.url.searchParams));
	/** The enterprise's local day by the server's clock, not the browser's UTC day. */
	const today = $derived(isoDay(serverClock.now()));
	const query = $derived(reportQuery(view, today));
	const moves = $derived(view.tab === 'moves');
	/** What is asked for the tab shown: the tabs never ask for each other's report. */
	const queryKey = $derived(JSON.stringify(moves ? [view.tab, movesQuery(view, today)] : query));
	const tabItems = [
		{ value: 'orders', label: t.orders },
		{ value: 'moves', label: t.moves }
	];
	/** An end earlier than the start is shown at the field «По»; there is nothing to ask. */
	const reversed = $derived(
		view.period === 'custom' && view.from !== null && view.to !== null && view.to < view.from
	);

	// The view the address is going to have: a change made before the last one has arrived builds on it.
	let pending: ReportView | null = null;
	function navigate(change: (v: ReportView) => ReportView) {
		const next = change(pending ?? view);
		const search = reportViewToSearch(next);
		if (search === reportViewToSearch(pending ?? view)) return;
		pending = next;
		const arrived = goto(search ? resolve(`reports?${search}`) : resolve('reports'), {
			reset: false
		});
		void arrived.finally(() => {
			if (pending === next) pending = null;
		});
	}

	let report = $state.raw<OrderReport | null>(null);
	let transfers = $state.raw<MovesReport | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let controller: AbortController | null = null;

	async function load(
		tab: ReportView['tab'],
		ask: typeof query,
		ask2: ReturnType<typeof movesQuery>
	) {
		controller?.abort();
		const mine = new AbortController();
		controller = mine;
		loading = true;
		error = null;
		try {
			if (tab === 'moves') {
				const next = await movesReport(ask2, mine.signal);
				if (controller !== mine) return;
				transfers = next;
			} else {
				const next = await orderReport(ask, mine.signal);
				if (controller !== mine) return;
				report = next;
			}
		} catch (e) {
			// Called off by a newer view: its answer is the one to show.
			if (controller !== mine || mine.signal.aborted) return;
			if (tab === 'moves') transfers = null;
			else report = null;
			error = errorMessage(e);
		} finally {
			if (controller === mine) loading = false;
		}
	}

	$effect(() => {
		void queryKey;
		if (!canView) return;
		if (reversed) {
			untrack(() => {
				controller?.abort();
				controller = null;
				report = null;
				transfers = null;
				error = null;
				loading = false;
			});
			return;
		}
		untrack(() => void load(view.tab, query, movesQuery(view, today)));
	});
	$effect(() => () => controller?.abort());

	let exporting = $state(false);
	async function exportFile() {
		exporting = true;
		try {
			await downloadOrderReport(query);
		} catch (e) {
			toast.show({
				message: `${t.exportFailed}: ${e instanceof ApiError ? e.message : errorMessage(e)}`,
				tone: 'danger'
			});
		} finally {
			exporting = false;
		}
	}
</script>

<svelte:head><title>{t.title}</title></svelte:head>

{#if !canView}
	<div class="reports">
		<h1 class="title">{t.title}</h1>
		<EmptyState icon={ChartColumn} title={t.noAccess} />
	</div>
{:else}
	<div class="reports" class:phone={display.phone}>
		<h1 class="title">{t.title}</h1>
		<Tabs
			items={tabItems}
			bind:value={
				() => view.tab,
				(tab) => navigate((v) => ({ ...v, tab: tab === 'moves' ? 'moves' : 'orders' }))
			}
		/>
		<ReportFilters {view} {today} {exporting} onchange={navigate} onexport={exportFile} />
		{#if error}
			<Callout tone="danger" title={t.loadFailed}>{error}</Callout>
		{/if}
		{#if moves ? transfers : report}
			<div class="body" class:stale={loading} aria-busy={loading}>
				{#if moves && transfers}
					<MovesReportView
						report={transfers}
						onorder={(id, ids) => void orderPanel.open(id, { neighbors: () => [...ids] })}
					/>
				{:else if report}
					<OrdersReport {report} groupBy={view.groupBy} />
				{/if}
			</div>
		{:else if loading}
			<Skeleton height="8rem" radius="md" />
		{/if}
	</div>
{/if}

<style>
	.reports {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		height: 100%;
		padding: var(--space-4);
		overflow-y: auto;
	}
	.reports.phone {
		padding: var(--space-3);
	}
	/* The heading is for screen readers and the window title. */
	.title {
		position: absolute;
		width: 1px;
		height: 1px;
		margin: -1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.body {
		min-width: 0;
	}
	/* While the new answer comes, the old one stays and fades. */
	.stale {
		opacity: 0.6;
	}
</style>
