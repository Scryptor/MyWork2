<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ComponentProps } from 'svelte';
	import DataTable from './DataTable.svelte';
	import type { RowId } from './types.js';

	let {
		selected = $bindable(new Set<RowId>()),
		...props
	}: ComponentProps<typeof DataTable<unknown>> = $props();
</script>

<Tooltip.Provider>
	<DataTable {...props} bind:selected />
</Tooltip.Provider>
<output data-testid="selected"
	>{[...selected].sort((a, b) => Number(a) - Number(b)).join(',')}</output
>
