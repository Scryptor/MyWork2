<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import PatternField from './PatternField.svelte';

	let {
		value = '',
		disabled = false,
		onchange
	}: { value?: string; disabled?: boolean; onchange?: (value: string) => void } = $props();

	/* svelte-ignore state_referenced_locally */
	let v = $state(value);
</script>

<!-- The «Что это?» icon has a tooltip: the app gives its provider. -->
<Tooltip.Provider>
	<PatternField
		label="Графический ключ"
		help="Необязательно."
		value={v}
		{disabled}
		onchange={(next) => {
			v = next;
			onchange?.(next);
		}}
	/>
	<output data-testid="out">{v}</output>
</Tooltip.Provider>
