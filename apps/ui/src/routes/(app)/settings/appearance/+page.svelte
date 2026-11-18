<!-- «Внешний вид» (spec 6.8): kept on this computer only; every field applies at once. -->
<script lang="ts">
	import { display, prefs } from '#lib/design/prefs.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { scaleLabel } from '#lib/settings/size.js';
	import Card from '#lib/ui/Card.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import { UI_SCALES } from '#lib/design/display.js';

	const t = ru.settings;
	const THEMES = (['system', 'light', 'dark'] as const).map((value) => ({
		value,
		label: ru.app.themes[value]
	}));
	const SIZES = [
		{ value: 'auto' as const, label: t.sizeAuto },
		...UI_SCALES.map((value) => ({ value, label: scaleLabel(value) }))
	];
	const DENSITIES = (['comfortable', 'compact'] as const).map((value) => ({
		value,
		label: t.densities[value]
	}));
</script>

<h1 class="title">{t.appearance}</h1>

<Card>
	<div class="fields">
		<p class="note">{t.thisComputer}</p>
		<div class="field">
			<span class="label">{t.theme}</span>
			<SegmentedControl
				label={t.theme}
				options={THEMES}
				bind:value={() => prefs.value.theme, (v) => prefs.set({ theme: v })}
			/>
		</div>
		<div class="field">
			<span class="label">{t.size}</span>
			<SegmentedControl
				label={t.size}
				options={SIZES}
				bind:value={() => prefs.value.scale, (v) => prefs.set({ scale: v })}
			/>
			{#if prefs.value.scale === 'auto'}
				<p class="note">{t.autoHint(scaleLabel(display.value.scale))}</p>
			{/if}
		</div>
		<div class="field">
			<span class="label">{t.density}</span>
			<SegmentedControl
				label={t.density}
				options={DENSITIES}
				bind:value={() => prefs.value.density, (v) => prefs.set({ density: v })}
			/>
		</div>
	</div>
</Card>

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.fields {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-4);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.label {
		font-size: var(--text-sm);
		font-weight: 600;
	}
	.note {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
