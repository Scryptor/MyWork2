<script lang="ts">
	import { Chart, PeriodPicker } from '#lib/ui/index.js';
	import { periodRange, type PeriodKind } from '#lib/ui/period.js';

	const rub = (kopecks: number) => `${Math.round(kopecks / 100).toLocaleString('ru-RU')} ₽`;

	let kind = $state<PeriodKind>('month');
	let from = $state<string | null>(null);
	let to = $state<string | null>(null);
	const range = $derived(kind === 'custom' ? [from, to] : periodRange(kind, '2026-11-25'));

	function change(k: PeriodKind, f: string | null, t: string | null) {
		kind = k;
		from = f;
		to = t;
	}
</script>

<h3>Столбики и линия</h3>
<Chart
	label="Выручка, затраты и прибыль по месяцам"
	categories={['Авг', 'Сен', 'Окт', 'Ноя']}
	series={[
		{
			name: 'Выручка',
			values: [420_000_00, 515_000_00, 480_000_00, 560_000_00],
			kind: 'bar',
			tone: 1
		},
		{
			name: 'Затраты',
			values: [260_000_00, 300_000_00, 290_000_00, 320_000_00],
			kind: 'bar',
			tone: 2
		},
		{
			name: 'Прибыль',
			values: [160_000_00, 215_000_00, 190_000_00, 240_000_00],
			kind: 'line',
			tone: 3
		}
	]}
	format={rub}
/>
<h3>Горизонтальные столбики</h3>
<Chart
	label="Выручка по видам работ"
	categories={['Замена экрана', 'Замена батареи', 'Диагностика', 'Чистка']}
	series={[
		{
			name: 'Выручка',
			values: [210_000_00, 120_000_00, 60_000_00, 35_000_00],
			kind: 'bar',
			tone: 1
		}
	]}
	format={rub}
	horizontal
	height={220}
/>
<h3>Выбор периода</h3>
<PeriodPicker {kind} {from} {to} onchange={change} />
<p class="range" data-testid="period-range">
	{range[0] ?? '…'} — {range[1] ?? '…'}
</p>

<style>
	.range {
		color: var(--text-2);
	}
</style>
