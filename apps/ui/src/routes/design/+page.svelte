<script lang="ts">
	import { prefs } from '#lib/design/prefs.svelte.js';
	import { SegmentedControl } from '#lib/ui/index.js';
	import Basics from './sections/Basics.svelte';
	import Fields from './sections/Fields.svelte';
	import Tokens from './sections/Tokens.svelte';

	const THEMES = [
		{ value: 'system', label: 'Как в системе' },
		{ value: 'light', label: 'Светлая' },
		{ value: 'dark', label: 'Тёмная' }
	] as const;
	const SCALES = [
		{ value: 0.9375, label: 'Мельче' },
		{ value: 1, label: 'Обычный' },
		{ value: 1.125, label: 'Крупнее' },
		{ value: 1.25, label: 'Крупный' }
	] as const;
	const DENSITIES = [
		{ value: 'compact', label: 'Компактно' },
		{ value: 'comfortable', label: 'Обычно' }
	] as const;
</script>

<header class="bar">
	<h1>Дизайн-система</h1>
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
</style>
