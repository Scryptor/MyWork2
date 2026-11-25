<!-- The «Где лежит» block for unit tests: the editor and the racks are given by the test. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { StockRack } from '$api/StockRack';
	import type { StockEditor } from './editor.svelte.js';
	import StockPlaces from './StockPlaces.svelte';

	let {
		editor,
		racks,
		canEdit = true
	}: { editor: StockEditor; racks: StockRack[] | null; canEdit?: boolean } = $props();
</script>

<Tooltip.Provider>
	<StockPlaces {editor} {racks} {canEdit} />
	<output data-testid="unplaced">{editor.item.unplaced}</output>
</Tooltip.Provider>
