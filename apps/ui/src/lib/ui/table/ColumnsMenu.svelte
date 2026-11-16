<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronUp from '@lucide/svelte/icons/chevron-up';
	import Columns3Cog from '@lucide/svelte/icons/columns-3-cog';
	import { tick } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';
	import Checkbox from '../Checkbox.svelte';
	import Popover from '../Popover.svelte';
	import type { Column, ColumnPrefs } from './types.js';

	let { columns, prefs = $bindable() }: { columns: Column<never>[]; prefs: ColumnPrefs } = $props();

	const uid = $props.id();
	const headers = $derived(new Map(columns.map((c) => [c.id, c.header])));
	const visibleCount = $derived(prefs.order.filter((id) => !prefs.hidden.includes(id)).length);

	function setVisible(id: string, visible: boolean) {
		const hidden = prefs.hidden.filter((x) => x !== id);
		prefs = { ...prefs, hidden: visible ? hidden : [...hidden, id] };
	}

	const buttonId = (dir: 'up' | 'down', id: string) => `${uid}-${dir}-${id}`;

	async function move(id: string, dir: 'up' | 'down') {
		const from = prefs.order.indexOf(id);
		const to = dir === 'up' ? from - 1 : from + 1;
		const other = prefs.order[to];
		if (from < 0 || other === undefined) return;
		const order = [...prefs.order];
		order[to] = id;
		order[from] = other;
		prefs = { ...prefs, order };
		// The row moved in the DOM and lost focus: keep it on the same button, or on the other one at an end.
		await tick();
		const same = document.getElementById(buttonId(dir, id));
		const opposite = document.getElementById(buttonId(dir === 'up' ? 'down' : 'up', id));
		const target = same instanceof HTMLButtonElement && !same.disabled ? same : opposite;
		target?.focus();
	}
</script>

<Popover width="18rem" align="end">
	{#snippet trigger(props)}
		<button
			{...props}
			type="button"
			class="trigger"
			aria-label={ru.table.columns}
			title={ru.table.columns}
		>
			<Columns3Cog size={16} aria-hidden="true" />
		</button>
	{/snippet}
	<p class="title label-caps" id="{uid}-title">{ru.table.columns}</p>
	<ul class="list" aria-labelledby="{uid}-title">
		{#each prefs.order as id, i (id)}
			{@const header = headers.get(id) ?? id}
			{@const visible = !prefs.hidden.includes(id)}
			<li class="item">
				<span class="check">
					<Checkbox
						label={header}
						bind:checked={() => visible, (v) => setVisible(id, v)}
						disabled={visible && visibleCount <= 1}
					/>
				</span>
				<button
					type="button"
					class="move"
					id={buttonId('up', id)}
					aria-label="{ru.table.moveUp}: {header}"
					disabled={i === 0}
					onclick={() => move(id, 'up')}
				>
					<ChevronUp size={16} aria-hidden="true" />
				</button>
				<button
					type="button"
					class="move"
					id={buttonId('down', id)}
					aria-label="{ru.table.moveDown}: {header}"
					disabled={i === prefs.order.length - 1}
					onclick={() => move(id, 'down')}
				>
					<ChevronDown size={16} aria-hidden="true" />
				</button>
			</li>
		{/each}
	</ul>
</Popover>

<style>
	.trigger,
	.move {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		box-sizing: border-box;
		width: var(--control-sm);
		height: var(--control-sm);
		padding: 0;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text-2);
		cursor: pointer;
	}
	.trigger {
		max-height: 100%;
	}
	.trigger:hover,
	.trigger[data-state='open'],
	.move:hover:not(:disabled) {
		background: var(--surface-3);
		color: var(--text);
	}
	.move:disabled {
		color: var(--text-disabled);
		cursor: not-allowed;
	}
	.title {
		margin: 0 0 var(--space-2);
	}
	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
		max-height: min(24rem, 60vh);
		margin: 0;
		padding: 0;
		overflow-y: auto;
		list-style: none;
	}
	.item {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}
	.check {
		flex: 1;
		min-width: 0;
	}
</style>
