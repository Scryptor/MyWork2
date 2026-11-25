<!-- The report inside the tooltip provider the application's layout gives its hints. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ComponentProps } from 'svelte';
	import OrdersReport from './OrdersReport.svelte';

	let props: ComponentProps<typeof OrdersReport> = $props();
</script>

<Tooltip.Provider>
	<OrdersReport {...props} />
</Tooltip.Provider>
