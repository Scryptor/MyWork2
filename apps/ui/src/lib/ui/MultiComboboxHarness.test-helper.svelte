<script lang="ts">
	import MultiCombobox from './MultiCombobox.svelte';

	let {
		selected = [],
		options,
		allowCustom = false,
		oncreate,
		limit,
		withChosen = false,
		error
	}: {
		selected?: string[];
		options: string[];
		allowCustom?: boolean;
		oncreate?: (value: string) => void;
		limit?: number;
		/** The options take in the chosen values, as the intake does (`mergeSuggestions`). */
		withChosen?: boolean;
		error?: string;
	} = $props();

	/* svelte-ignore state_referenced_locally */
	let v = $state(selected);
	let pending = $state('');
</script>

<MultiCombobox
	bind:selected={v}
	bind:pending
	options={withChosen ? [...options, ...v.filter((x) => !options.includes(x))] : options}
	label="Комплектность"
	{allowCustom}
	{oncreate}
	{limit}
	{error}
/>
<output data-testid="out">{JSON.stringify(v)}</output>
<output data-testid="pending">{JSON.stringify(pending)}</output>
