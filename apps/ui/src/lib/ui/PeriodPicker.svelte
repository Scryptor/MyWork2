<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import DateInput from './DateInput.svelte';
	import type { PeriodKind } from './period.js';
	import SegmentedControl from './SegmentedControl.svelte';

	let {
		kind,
		from,
		to,
		onchange
	}: {
		kind: PeriodKind;
		from: string | null;
		to: string | null;
		/** A quick period reports `null, null` (the screen computes the days); `custom` carries both. */
		onchange: (kind: PeriodKind, from: string | null, to: string | null) => void;
	} = $props();

	const options = (['today', 'week', 'month', 'last_month', 'year', 'all', 'custom'] as const).map(
		(value) => ({ value, label: ru.period[value] })
	);

	const reversed = $derived(kind === 'custom' && from !== null && to !== null && to < from);

	function choose(next: PeriodKind) {
		if (next === 'custom') onchange('custom', from, to);
		else onchange(next, null, null);
	}
</script>

<div class="period">
	<SegmentedControl label={ru.period.label} {options} bind:value={() => kind, (v) => choose(v)} />
	{#if kind === 'custom'}
		<div class="dates">
			<DateInput label={ru.period.from} bind:value={() => from, (v) => onchange('custom', v, to)} />
			<DateInput
				label={ru.period.to}
				error={reversed ? ru.period.reversed : undefined}
				bind:value={() => to, (v) => onchange('custom', from, v)}
			/>
		</div>
	{/if}
</div>

<style>
	.period {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		align-items: flex-start;
	}
	.dates {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
	}
</style>
