<!-- The place dialog of a cell for unit tests. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { StockCellRef } from '$api/StockCellRef';
	import type { StockItem } from '$api/StockItem';
	import PlaceDialog from './PlaceDialog.svelte';

	let {
		onsubmit,
		reload
	}: {
		onsubmit: (id: number, ref: StockCellRef, quantity: number) => Promise<void>;
		reload: (id: number) => Promise<StockItem>;
	} = $props();
	let open = $state(true);
</script>

<Tooltip.Provider>
	<PlaceDialog
		bind:open
		mode="here"
		racks={[]}
		cell={{ ref: { rack_id: 2, shelf: 2, cell: 4 }, code: 'B-2-4' }}
		{onsubmit}
		{reload}
	/>
</Tooltip.Provider>
