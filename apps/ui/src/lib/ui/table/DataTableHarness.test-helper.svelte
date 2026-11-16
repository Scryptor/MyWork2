<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ComponentProps } from 'svelte';
	import DataTable from './DataTable.svelte';

	// The layout of the app provides the tooltips; a bare table in a test needs it too.
	let props: ComponentProps<typeof DataTable<unknown>> = $props();
</script>

<Tooltip.Provider>
	<DataTable {...props} />
</Tooltip.Provider>
