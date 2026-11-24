<!-- The add dialog for unit tests: the tooltips of its hints need their provider, as in the app. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import StockAddDialog from './StockAddDialog.svelte';

	let { legacyPending = false }: { legacyPending?: boolean } = $props();
	let open = $state(true);
	let created = $state('');
</script>

<Tooltip.Provider>
	<StockAddDialog bind:open {legacyPending} oncreated={(item) => (created = String(item.id))} />
	<output data-testid="created">{created}</output>
</Tooltip.Provider>
