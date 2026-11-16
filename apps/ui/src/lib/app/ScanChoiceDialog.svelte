<!-- One barcode, several orders (old MyWork 2 codes repeat): which one to open (spec 6.10).
     ↑↓ move between the orders, Enter opens the focused one. -->
<script lang="ts">
	import type { OrderListItem } from '$api/OrderListItem';
	import { serverClock } from '#lib/api/server-clock.svelte.js';
	import { formatFull, formatRelative, shortName } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { deviceOf } from './palette.js';

	let {
		open = $bindable(false),
		code,
		orders,
		onchoose
	}: {
		open: boolean;
		code: string;
		orders: OrderListItem[];
		onchoose: (id: number) => void;
	} = $props();

	const now = $derived(open ? serverClock.now() : null);

	function choose(id: number) {
		open = false;
		onchoose(id);
	}

	let list = $state<HTMLElement | null>(null);

	function onkeydown(e: KeyboardEvent) {
		const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
		if (step === 0 && e.key !== 'Home' && e.key !== 'End') return;
		const rows = [...(list?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
		const at = rows.indexOf(e.currentTarget as HTMLButtonElement);
		const to =
			e.key === 'Home'
				? 0
				: e.key === 'End'
					? rows.length - 1
					: Math.min(rows.length - 1, Math.max(0, at + step));
		e.preventDefault();
		rows[to]?.focus();
	}
</script>

<Dialog bind:open title={ru.scan.chooseTitle} description={ru.scan.chooseDescription(code)}>
	<ul class="rows" bind:this={list}>
		{#each orders as o (o.id)}
			<li>
				<button type="button" class="row" onclick={() => choose(o.id)} {onkeydown}>
					<span class="num strong">№ {o.id}</span>
					<span class="muted" title={formatFull(o.received_at)}
						>{now ? formatRelative(o.received_at, now) : ''}</span
					>
					<span class="strong">{deviceOf(o)}</span>
					<span>{shortName(o.client_name)}</span>
				</button>
			</li>
		{/each}
	</ul>
</Dialog>

<style>
	.rows {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.row {
		display: grid;
		grid-template-columns: 5rem 8rem minmax(0, 1fr) minmax(0, 1fr);
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		width: 100%;
		min-height: var(--control-lg);
		padding: 0 var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.row:hover,
	.row:focus-visible {
		background: var(--accent-soft);
		color: var(--accent-text);
	}
	.row > span {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.strong {
		font-weight: 500;
	}
	.muted {
		color: var(--text-3);
	}
	@media (max-width: 599px) {
		.row {
			grid-template-columns: 5rem minmax(0, 1fr);
			padding-block: var(--space-2);
		}
	}
	@media (pointer: coarse) {
		.row {
			min-height: 2.75rem;
		}
	}
</style>
