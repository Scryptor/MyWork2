<!-- The rack dialog for unit tests: the tooltips of its hints need their provider, as in the app. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ServicePoint } from '$api/ServicePoint';
	import type { StockRack } from '$api/StockRack';
	import RackDialog from './RackDialog.svelte';

	let {
		rack,
		points = [],
		onsaved = () => {}
	}: { rack?: StockRack; points?: ServicePoint[]; onsaved?: (r: StockRack) => void } = $props();
	let open = $state(true);
</script>

<Tooltip.Provider>
	<RackDialog bind:open {rack} {points} {onsaved} />
	<output data-testid="open">{open}</output>
</Tooltip.Provider>
