<!-- The panel of a cell for unit tests: the tooltips need their provider, as in the app. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { StockRack } from '$api/StockRack';
	import CellPanel from './CellPanel.svelte';

	let {
		racks,
		canEdit = true,
		onchanged = () => {},
		onopenitem = () => {}
	}: {
		racks: StockRack[];
		canEdit?: boolean;
		onchanged?: () => void;
		onopenitem?: (id: number) => void;
	} = $props();
</script>

<Tooltip.Provider>
	<CellPanel
		target={{ ref: { rack_id: 2, shelf: 2, cell: 4 }, code: 'B-2-4' }}
		{racks}
		{canEdit}
		onclose={() => {}}
		{onchanged}
		{onopenitem}
	/>
</Tooltip.Provider>
