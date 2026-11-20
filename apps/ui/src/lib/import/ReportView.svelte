<!-- The report of a transfer from MyWork 2 (spec 6.9): tiles of totals; the groups of «Требует внимания»
     (then «Сведения»). Each group has «Что это?» with the text of its code and opens into the numbers of its
     orders (a number opens the order's panel) and the entries of its warnings without an order (clients,
     history, files: the first 200); «Сохранить отчёт» (the full text as a file) and «К заказам». -->
<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Download from '@lucide/svelte/icons/download';
	import { resolve } from '$app/paths';
	import type { ImportReportView } from '$api/ImportReportView';
	import { formatFull } from '#lib/format/date.js';
	import { groupDigits } from '#lib/format/digits.js';
	import { ru } from '#lib/i18n/ru.js';
	import { downloadText } from '#lib/platform/download.js';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import StatTile from '#lib/ui/StatTile.svelte';
	import {
		entryWhere,
		ORDERS_SHOWN,
		reportFileName,
		reportSections,
		reportTiles,
		visibleOrders
	} from './report.js';

	let {
		report,
		onorder
	}: {
		report: ImportReportView;
		/** A number of a group was clicked: `ids` are the numbers of that group, to go along. */
		onorder: (id: number, ids: readonly number[]) => void;
	} = $props();

	const t = ru.legacy;
	const r = ru.importReport;
	const uid = $props.id();
	const tiles = $derived(reportTiles(report));
	const sections = $derived(reportSections(report.groups));
	let open = $state<Record<string, boolean>>({});
	let all = $state<Record<string, boolean>>({});
</script>

<div class="report">
	<header class="head">
		<h1>{t.report}</h1>
		<p class="meta">
			<span>{t.reportOf(formatFull(report.imported_at))}</span>
			{#if report.mode === 'replace'}<Badge tone="warning">{t.replaced}</Badge>{/if}
		</p>
		<p class="meta">
			<span class="label">{t.from}:</span> <span class="path">{report.source_path}</span>
		</p>
	</header>

	{#if report.files_pending}<Callout tone="warning">{t.filesPending}</Callout>{/if}

	<ul class="tiles" aria-label={t.totals}>
		{#each tiles as tile (tile.label)}
			<li><StatTile {...tile} /></li>
		{/each}
	</ul>

	{#each sections as section (section.id)}
		<section class="section" aria-labelledby="{uid}-{section.id}">
			<h2 id="{uid}-{section.id}">{section.title}</h2>
			<ul class="groups">
				{#each section.groups as group (group.code)}
					{@const expanded = open[group.code] === true}
					{@const expandable = group.order_ids.length > 0 || group.entries.length > 0}
					<li class="group">
						<div class="line">
							{#if expandable}
								<button
									type="button"
									class="row"
									aria-expanded={expanded}
									aria-controls="{uid}-{group.code}"
									onclick={() => (open[group.code] = !expanded)}
								>
									<span class="chevron" class:expanded aria-hidden="true"
										><ChevronRight size={16} /></span
									>
									<span class="title">{group.title}</span>
									<span class="count num">{groupDigits(String(group.count))}</span>
								</button>
							{:else}
								<div class="row static">
									<span class="chevron" aria-hidden="true"></span>
									<span class="title">{group.title}</span>
									<span class="count num">{groupDigits(String(group.count))}</span>
								</div>
							{/if}
							<span class="hint"><HelpHint title={group.title} text={r.explain[group.code]} /></span
							>
						</div>
						{#if expandable && expanded}
							<div class="orders" id="{uid}-{group.code}">
								{#if group.order_ids.length > 0}
									<ul class="numbers" aria-label={t.groupOrders(group.title)}>
										{#each visibleOrders(group.order_ids, all[group.code] === true) as id (id)}
											<li>
												<button
													type="button"
													class="order num"
													onclick={() => onorder(id, group.order_ids)}>{t.orderNumber(id)}</button
												>
											</li>
										{/each}
									</ul>
									{#if all[group.code] !== true && group.order_ids.length > ORDERS_SHOWN}
										<Button size="sm" variant="ghost" onclick={() => (all[group.code] = true)}
											>{t.showAll(group.order_ids.length)}</Button
										>
									{/if}
								{/if}
								{#if group.entries.length > 0}
									<ul class="entries" aria-label={r.entries(group.title)}>
										{#each group.entries as entry, i (i)}
											<li class="entry">
												<span class="where num">{entryWhere(entry, group.code)}</span>
												{#if entry.value !== ''}
													<span class="value"
														><span class="label">{r.value}:</span> {entry.value}</span
													>
												{/if}
												{#if entry.message !== ''}<span class="message">{entry.message}</span>{/if}
											</li>
										{/each}
									</ul>
									{#if group.entries.length < group.entries_count}
										<p class="cut">{r.cut(group.entries.length, group.entries_count)}</p>
									{/if}
								{/if}
							</div>
						{/if}
					</li>
				{/each}
			</ul>
		</section>
	{:else}
		<Callout tone="success">{t.nothing}</Callout>
	{/each}

	<div class="actions">
		<Button
			icon={Download}
			onclick={() => downloadText(reportFileName(report), report.warnings_text)}>{t.save}</Button
		>
		<Button variant="primary" href={resolve('orders')}>{t.toOrders}</Button>
	</div>
</div>

<style>
	.report {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 0;
	}
	.head {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	h1 {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin: 0;
		color: var(--text-2);
		font-size: var(--text-md);
	}
	.label {
		color: var(--text-3);
	}
	.path {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	h2 {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.groups {
		margin: 0;
		padding: 0;
		list-style: none;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
	}
	.group + .group {
		border-top: 1px solid var(--border);
	}
	.line {
		display: flex;
		align-items: center;
	}
	.hint {
		flex: none;
		padding-inline-end: var(--space-2);
	}
	.row {
		display: flex;
		flex: 1;
		min-width: 0;
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		min-height: var(--control-lg);
		padding: var(--space-1-5) var(--space-3);
		border: 0;
		background: transparent;
		color: var(--text);
		font: inherit;
		font-size: var(--text-md);
		text-align: start;
	}
	button.row {
		cursor: pointer;
	}
	button.row:hover {
		background: var(--surface-3);
	}
	.chevron {
		display: inline-flex;
		flex: none;
		width: 1rem;
		color: var(--text-3);
		transition: transform var(--duration-fast);
	}
	.chevron.expanded {
		transform: rotate(90deg);
	}
	.title {
		flex: 1;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.count {
		flex: none;
		color: var(--text-2);
	}
	.orders {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
		padding: 0 var(--space-3) var(--space-3) calc(var(--space-3) + 1rem + var(--space-2));
	}
	.orders ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.numbers {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1-5);
	}
	.entries {
		display: flex;
		flex-direction: column;
		align-self: stretch;
		gap: var(--space-1-5);
	}
	.entry {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
		min-width: 0;
		overflow-wrap: anywhere;
		font-size: var(--text-md);
	}
	.where {
		color: var(--text);
	}
	.value,
	.message {
		color: var(--text-2);
	}
	.cut {
		margin: 0;
		color: var(--text-2);
		font-size: var(--text-md);
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
	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
