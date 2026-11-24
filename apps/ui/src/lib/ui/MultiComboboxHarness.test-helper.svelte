<script lang="ts">
	import MultiCombobox from './MultiCombobox.svelte';

	let {
		selected = [],
		options,
		allowCustom = false,
		oncreate,
		limit,
		withChosen = false
	}: {
		selected?: string[];
		options: string[];
		allowCustom?: boolean;
		oncreate?: (value: string) => void;
		limit?: number;
		/** The options take in the chosen values, as the intake does (`mergeSuggestions`). */
		withChosen?: boolean;
	} = $props();

	/* svelte-ignore state_referenced_locally */
	let v = $state(selected);
</script>

<MultiCombobox
	bind:selected={v}
	options={withChosen ? [...options, ...v.filter((x) => !options.includes(x))] : options}
	label="Комплектность"
	{allowCustom}
	{oncreate}
	{limit}
/>
<output data-testid="out">{JSON.stringify(v)}</output>
