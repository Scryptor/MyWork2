<script lang="ts">
	import type { StatusHue } from '#lib/design/status-colors.js';
	import { ColorSwatches, SortableList, StatusBadge } from '#lib/ui/index.js';

	type Item = { id: number; name: string; color: StatusHue };
	let items = $state<Item[]>([
		{ id: 1, name: 'Принят', color: 'blue' },
		{ id: 2, name: 'Диагностика', color: 'amber' },
		{ id: 3, name: 'Готов', color: 'green' },
		{ id: 4, name: 'Выдан', color: 'gray' }
	]);
	let hue = $state<StatusHue>('violet');

	function reorder(ids: number[]) {
		items = ids.map((id) => items.find((i) => i.id === id)!);
	}
</script>

<h3>Сортируемый список</h3>
<div class="box" data-testid="sortable-demo">
	<SortableList {items} key={(i) => i.id} label={(i) => i.name} onreorder={reorder}>
		{#snippet row(i)}<StatusBadge name={i.name} color={i.color} />{/snippet}
	</SortableList>
</div>
<h3>Выбор цвета</h3>
<ColorSwatches label="Цвет статуса" value={hue} onchange={(h) => (hue = h)} />

<style>
	.box {
		max-width: 22rem;
		margin-bottom: var(--space-4);
	}
</style>
