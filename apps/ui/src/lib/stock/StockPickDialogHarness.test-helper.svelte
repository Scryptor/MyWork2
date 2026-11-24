<!-- The pick dialog for unit tests: the parts of an order are given by the test. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { OrderPartsState } from '#lib/orders/parts.svelte.js';
	import StockPickDialog from './StockPickDialog.svelte';

	let { parts }: { parts: OrderPartsState } = $props();
	let open = $state(true);
</script>

<Tooltip.Provider>
	<StockPickDialog bind:open {parts} />
	<output data-testid="open">{open}</output>
</Tooltip.Provider>
