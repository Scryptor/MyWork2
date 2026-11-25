<!-- The transfers of orders between the points: the routes with their counts, then the list of the latest
     transfers. A number of an order opens its panel (the same button as in the report of an import). -->
<script lang="ts">
	import type { MovesReport } from '$api/MovesReport';
	import { formatFull } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import EmptyState from '#lib/ui/EmptyState.svelte';

	let {
		report,
		onorder
	}: {
		report: MovesReport;
		/** A number was clicked: `ids` are the orders of the list, to go along. */
		onorder: (id: number, ids: readonly number[]) => void;
	} = $props();

	const t = ru.reports;
	// An order that moved twice is one neighbour: the first occurrence stays.
	const ids = $derived([...new Set(report.items.map((i) => i.order_id))]);
</script>

{#if report.total === 0}
	<EmptyState title={t.movesEmpty} />
{:else}
	<div class="report">
		<section class="block scroll">
			<table aria-label={t.movesRoutes}>
				<thead>
					<tr>
						<th scope="col" class="text">{t.movesRoute}</th>
						<th scope="col">{t.movesCount}</th>
					</tr>
				</thead>
				<tbody>
					{#each report.routes as route (`${route.from}>${route.to}`)}
						<tr>
							<th scope="row" class="text">{t.movesRouteName(route.from_name, route.to_name)}</th>
							<td class="num">{route.count}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</section>

		<section class="block scroll">
			<table aria-label={t.movesList}>
				<thead>
					<tr>
						<th scope="col" class="text">{t.movesWhen}</th>
						<th scope="col" class="text">{t.movesOrder}</th>
						<th scope="col" class="text">{t.movesFrom}</th>
						<th scope="col" class="text">{t.movesTo}</th>
						<th scope="col" class="text">{t.movesWho}</th>
					</tr>
				</thead>
				<tbody>
					{#each report.items as item, i (i)}
						<tr>
							<td class="text num">{formatFull(item.at)}</td>
							<td class="text">
								<button type="button" class="order num" onclick={() => onorder(item.order_id, ids)}
									>{t.orderNumber(item.order_id)}</button
								>
							</td>
							<td class="text">{item.from_name}</td>
							<td class="text">{item.to_name}</td>
							<td class="text">{item.user_name}</td>
						</tr>
					{/each}
				</tbody>
			</table>
			{#if report.total > report.items.length}
				<p class="cut">{t.movesCut(report.items.length, report.total)}</p>
			{/if}
		</section>
	</div>
{/if}

<style>
	.report {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 0;
	}
	.block {
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
	}
	/* The tables scroll inside their blocks; the page does not scroll sideways. */
	.scroll {
		overflow-x: auto;
	}
	table {
		width: 100%;
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
	.cut {
		margin: 0;
		padding: var(--space-2) var(--space-3);
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.order {
		box-sizing: border-box;
		min-height: var(--control-sm);
		padding: 0 var(--space-2);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-sm);
		background: var(--surface);
		color: var(--accent-text);
		font: inherit;
		font-size: var(--text-md);
		white-space: nowrap;
		cursor: pointer;
	}
	.order:hover {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
</style>
