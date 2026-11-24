<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import { patternText } from '#lib/orders/pattern.js';
	import { PatternField, PatternInput } from '#lib/ui/index.js';

	let grid = $state('');
	let empty = $state('');
	let given = $state('15963');
</script>

<h3>Сетка</h3>
<p class="muted">
	Точки нажимаются по порядку; шаг через точку берёт и её (1 → 3 — это 1 · 2 · 3). Ключ — от 4
	точек.
</p>
<PatternInput bind:value={grid} />
<p class="muted">Значение: {grid || 'пусто'}</p>

<h3>Поле</h3>
<p class="muted">
	Сетка открывается во всплывающем окне; «Готово» принимает ключ, закрытие без него ничего не
	меняет.
</p>
<div class="grid">
	<PatternField
		label="Графический ключ"
		help={ru.help.intake.pattern}
		value={empty}
		onchange={(v) => (empty = v)}
	/>
	<PatternField label="Ключ задан" value={given} onchange={(v) => (given = v)} />
	<PatternField label="Ключ отключённый" value="1596" disabled onchange={() => {}} />
</div>
<p class="muted">Ключ задан: {patternText(given) || 'нет'}</p>

<style>
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
		gap: var(--space-4);
		align-items: start;
		margin-bottom: var(--space-4);
	}
	.muted {
		color: var(--text-3);
	}
</style>
