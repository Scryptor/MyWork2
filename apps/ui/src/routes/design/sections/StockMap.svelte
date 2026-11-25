<script lang="ts">
	import { CellGrid } from '#lib/ui/index.js';

	// 3 shelves of 4 cells: empty ones, occupied ones, a found one and the chosen one.
	const positions = [0, 3, 0, 1, 0, 0, 12, 0, 2, 0, 0, 0];
	const found = new Set(['B-3-1']);
	const cells = positions.map((count, i) => {
		const shelf = Math.floor(i / 4) + 1;
		const cell = (i % 4) + 1;
		const code = `B-${shelf}-${cell}`;
		return { shelf, cell, code, positions: count, found: found.has(code) };
	});
	let selected = $state<string | null>('B-2-3');
</script>

<h3>Ячейки шкафа</h3>
<div class="box" data-testid="cell-grid-demo">
	<CellGrid
		{cells}
		shelves={3}
		perShelf={4}
		{selected}
		onselect={(c) => (selected = c)}
		label="Шкаф B"
	/>
</div>
<p class="hint">Выбрана ячейка: {selected ?? 'никакая'}</p>

<style>
	.box {
		max-width: 24rem;
	}
	.hint {
		color: var(--text-3);
	}
</style>
