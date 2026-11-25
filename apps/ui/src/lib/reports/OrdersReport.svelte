<!-- The report of the issued orders: the tiles, the two charts (by time and by the grouping), the table
     of the groups with its total. The numbers are kopecks as the server counts them. -->
<script lang="ts">
	import type { OrderReport } from '$api/OrderReport';
	import type { OrderReportGroup } from '$api/OrderReportGroup';
	import type { ReportGroupBy } from '$api/ReportGroupBy';
	import type { ReportTotals } from '$api/ReportTotals';
	import { formatDayMonth, formatMonthYear } from '#lib/format/date.js';
	import { formatMoney } from '#lib/format/money.js';
	import { ru } from '#lib/i18n/ru.js';
	import Chart from '#lib/ui/Chart.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import StatTile from '#lib/ui/StatTile.svelte';

	let { report, groupBy }: { report: OrderReport; groupBy: ReportGroupBy } = $props();

	const t = ru.reports;
	/** The groups drawn one by one; the rest is one bar. */
	const CHART_GROUPS = 10;
	const GROUP_TITLE: Record<ReportGroupBy, string> = {
		master: t.groupByMaster,
		service_point: t.groupByPoint,
		device_type: t.groupByType,
		brand: t.groupByBrand,
		source: t.groupBySource
	};

	const totals = $derived(report.totals);
	const average = $derived(totals.orders > 0 ? Math.round(totals.profit / totals.orders) : 0);

	const sum = (list: ReportTotals[]): ReportTotals =>
		list.reduce(
			(a, x) => ({
				orders: a.orders + x.orders,
				without_repair: a.without_repair + x.without_repair,
				revenue: a.revenue + x.revenue,
				discounts: a.discounts + x.discounts,
				costs: a.costs + x.costs,
				profit: a.profit + x.profit,
				master_pay: a.master_pay + x.master_pay
			}),
			{ orders: 0, without_repair: 0, revenue: 0, discounts: 0, costs: 0, profit: 0, master_pay: 0 }
		);

	/** The first groups and, when there are more, «Остальные» with the sum of the rest. */
	const chartGroups = $derived.by<Pick<OrderReportGroup, 'label' | 'totals'>[]>(() => {
		const all = report.groups;
		if (all.length <= CHART_GROUPS) return all;
		return [
			...all.slice(0, CHART_GROUPS),
			{ label: t.others, totals: sum(all.slice(CHART_GROUPS).map((g) => g.totals)) }
		];
	});

	const timeLabel = $derived(report.step === 'month' ? formatMonthYear : formatDayMonth);
	const timeCategories = $derived(report.timeline.map((p) => timeLabel(p.start)));
	const timeSeries = $derived([
		{
			name: t.revenue,
			kind: 'bar' as const,
			tone: 1 as const,
			values: report.timeline.map((p) => p.totals.revenue)
		},
		{
			name: t.costs,
			kind: 'bar' as const,
			tone: 2 as const,
			values: report.timeline.map((p) => p.totals.costs)
		},
		{
			name: t.profit,
			kind: 'line' as const,
			tone: 3 as const,
			values: report.timeline.map((p) => p.totals.profit)
		}
	]);
	const groupSeries = $derived([
		{
			name: t.revenue,
			kind: 'bar' as const,
			tone: 1 as const,
			values: chartGroups.map((g) => g.totals.revenue)
		},
		{
			name: t.costs,
			kind: 'bar' as const,
			tone: 2 as const,
			values: chartGroups.map((g) => g.totals.costs)
		},
		{
			name: t.profit,
			kind: 'bar' as const,
			tone: 3 as const,
			values: chartGroups.map((g) => g.totals.profit)
		}
	]);
	/** A bar of three needs room: the chart grows with the number of groups. */
	const groupHeight = $derived(Math.max(200, chartGroups.length * 64 + 40));
	const money = (kopecks: number) => formatMoney(kopecks);
</script>

<div class="report">
	<div class="tiles" role="group" aria-label={t.tiles}>
		<div class="tile">
			<StatTile
				label={t.ordersTile}
				value={String(totals.orders)}
				hint={t.withoutRepair(totals.without_repair)}
			/>
			<span class="help"><HelpHint title={t.ordersTile} text={t.ordersHint} /></span>
		</div>
		<StatTile
			label={t.revenue}
			value={money(totals.revenue)}
			hint={t.discounts(money(totals.discounts))}
		/>
		<StatTile label={t.costs} value={money(totals.costs)} hint={t.costsHint} />
		<StatTile
			label={t.profit}
			value={money(totals.profit)}
			hint={t.profitAverage(money(average))}
		/>
		<StatTile
			label={t.masterPay}
			value={money(totals.master_pay)}
			hint={t.masterPayHint(report.master_percent)}
		/>
	</div>

	{#if totals.orders === 0}
		<EmptyState title={t.empty} />
	{:else}
		<section class="block">
			<h2>{t.timeline}</h2>
			<Chart label={t.timeline} categories={timeCategories} series={timeSeries} format={money} />
		</section>
		<section class="block">
			<h2>{GROUP_TITLE[groupBy]}</h2>
			<Chart
				label={GROUP_TITLE[groupBy]}
				categories={chartGroups.map((g) => g.label)}
				series={groupSeries}
				horizontal
				format={money}
				height={groupHeight}
			/>
		</section>
		<!-- A named section is a region; the keyboard reaches the scrolling table through it. -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<section class="block scroll" aria-label={t.groupsRegion} tabindex="0">
			<table aria-label={t.groupsTable}>
				<thead>
					<tr>
						<th scope="col" class="text">{GROUP_TITLE[groupBy]}</th>
						<th scope="col">{t.columnOrders}</th>
						<th scope="col">{t.columnWithoutRepair}</th>
						<th scope="col">{t.columnRevenue}</th>
						<th scope="col">{t.columnCosts}</th>
						<th scope="col">{t.columnProfit}</th>
						<th scope="col">{t.columnMasterPay}</th>
					</tr>
				</thead>
				<tbody>
					{#each report.groups as g (g.key)}
						{@render line(g.label, g.totals)}
					{/each}
				</tbody>
				<tfoot>
					{@render line(t.total, totals)}
				</tfoot>
			</table>
		</section>
	{/if}
</div>

{#snippet line(label: string, x: ReportTotals)}
	<tr>
		<th scope="row" class="text">{label}</th>
		<td class="num">{x.orders}</td>
		<td class="num">{x.without_repair}</td>
		<td class="num">{money(x.revenue)}</td>
		<td class="num">{money(x.costs)}</td>
		<td class="num">{money(x.profit)}</td>
		<td class="num">{money(x.master_pay)}</td>
	</tr>
{/snippet}

<style>
	.report {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 0;
	}
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
		gap: var(--space-3);
	}
	.tile {
		position: relative;
		min-width: 0;
	}
	.help {
		position: absolute;
		top: var(--space-2);
		right: var(--space-2);
	}
	.block {
		min-width: 0;
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
	}
	h2 {
		margin: 0 0 var(--space-2);
		font-size: var(--text-md);
		font-weight: 600;
	}
	/* The table scrolls inside its block; the page does not scroll sideways. */
	.scroll {
		overflow-x: auto;
		padding: 0;
	}
	table {
		width: 100%;
		min-width: 40rem;
		border-collapse: collapse;
	}
	th,
	td {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
		text-align: right;
		white-space: nowrap;
	}
	thead th {
		color: var(--text-3);
		font-size: var(--text-sm);
		font-weight: 500;
	}
	.text {
		text-align: left;
	}
	tbody th {
		font-weight: 500;
	}
	tfoot th,
	tfoot td {
		border-bottom: 0;
		font-weight: 600;
	}
</style>
