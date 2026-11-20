<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import ChipGroup from './ChipGroup.svelte';
	import Field from './Field.svelte';
	import HelpHint from './HelpHint.svelte';
	import Section from './Section.svelte';
	import Switch from './Switch.svelte';

	let {
		kind,
		text = 'Пояснение'
	}: {
		kind: 'hint' | 'field' | 'switch' | 'section' | 'chips';
		text?: string | readonly string[];
	} = $props();
	let checked = $state(false);
	let chips = $state<string[]>([]);
</script>

<Tooltip.Provider>
	{#if kind === 'hint'}
		<HelpHint title="Валюта" {text} />
	{:else if kind === 'field'}
		<Field id="x" label="Имя" help={text}>
			{#snippet children(f)}<input id={f.id} />{/snippet}
		</Field>
		<Field id="y" label="Без подсказки">
			{#snippet children(f)}<input id={f.id} />{/snippet}
		</Field>
	{:else if kind === 'switch'}
		<Switch bind:checked label="Включено" help={text} />
	{:else if kind === 'chips'}
		<ChipGroup bind:selected={chips} options={['Чехол']} label="Комплектность" help={text} />
	{:else}
		<Section title="Требуют внимания" help={text}>
			<div></div>
		</Section>
	{/if}
</Tooltip.Provider>
