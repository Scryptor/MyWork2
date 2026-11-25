<script lang="ts">
	import { resolve } from '$app/paths';
	import { prefs } from '#lib/design/prefs.svelte.js';
	import { SegmentedControl } from '#lib/ui/index.js';
	import Basics from './sections/Basics.svelte';
	import Charts from './sections/Charts.svelte';
	import Fields from './sections/Fields.svelte';
	import Layers from './sections/Layers.svelte';
	import Media from './sections/Media.svelte';
	import Ordering from './sections/Ordering.svelte';
	import Pattern from './sections/Pattern.svelte';
	import Navigation from './sections/Navigation.svelte';
	import Print from './sections/Print.svelte';
	import StockMap from './sections/StockMap.svelte';
	import Table from './sections/Table.svelte';
	import Tokens from './sections/Tokens.svelte';

	const THEMES = [
		{ value: 'system', label: 'Как в системе' },
		{ value: 'light', label: 'Светлая' },
		{ value: 'dark', label: 'Тёмная' }
	] as const;
	const SCALES = [
		{ value: 'auto', label: 'Авто' },
		{ value: 0.9, label: '90 %' },
		{ value: 1, label: '100 %' },
		{ value: 1.1, label: '110 %' },
		{ value: 1.25, label: '125 %' },
		{ value: 1.5, label: '150 %' }
	] as const;
	const DENSITIES = [
		{ value: 'compact', label: 'Компактно' },
		{ value: 'comfortable', label: 'Обычно' }
	] as const;
</script>

<!-- tabindex: focus lands here when the element that opened a dialog is gone. -->
<main tabindex="-1" aria-labelledby="design-title">
	<header class="bar">
		<h1 id="design-title">Дизайн-система</h1>
		<nav class="demos" aria-label="Рабочие экраны">
			<a href={resolve('/(app)/orders')}>Заказы</a>
		</nav>
		<div class="prefs">
			<SegmentedControl
				label="Тема"
				options={[...THEMES]}
				bind:value={() => prefs.value.theme, (v) => prefs.set({ theme: v })}
			/>
			<SegmentedControl
				label="Размер текста"
				options={[...SCALES]}
				bind:value={() => prefs.value.scale, (v) => prefs.set({ scale: v })}
			/>
			<SegmentedControl
				label="Плотность"
				options={[...DENSITIES]}
				bind:value={() => prefs.value.density, (v) => prefs.set({ density: v })}
			/>
		</div>
	</header>

	<section id="tokens">
		<h2>Токены</h2>
		<Tokens />
	</section>

	<section id="basics">
		<h2>Основа</h2>
		<Basics />
	</section>

	<section id="fields">
		<h2>Поля</h2>
		<Fields />
	</section>

	<section id="pattern">
		<h2>Графический ключ</h2>
		<Pattern />
	</section>

	<section id="layers">
		<h2>Слои</h2>
		<Layers />
	</section>

	<section id="navigation">
		<h2>Навигация</h2>
		<Navigation />
	</section>

	<section id="table">
		<h2>Таблица</h2>
		<Table />
	</section>

	<section id="ordering">
		<h2>Порядок и цвет</h2>
		<Ordering />
	</section>

	<section id="charts">
		<h2>Графики и период</h2>
		<Charts />
	</section>

	<section id="media">
		<h2>Медиа</h2>
		<Media />
	</section>

	<section id="print">
		<h2>Печать</h2>
		<Print />
	</section>

	<section id="stock-map">
		<h2>Карта склада</h2>
		<StockMap />
	</section>
</main>

<style>
	.bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
	}
	.prefs {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.demos {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
	}
	.demos a {
		color: var(--accent);
		font-weight: 500;
	}
</style>
