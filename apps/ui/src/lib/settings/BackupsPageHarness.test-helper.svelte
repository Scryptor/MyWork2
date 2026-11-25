<!-- «Резервные копии» for unit tests: the tooltips of the hints need their provider, as in the app. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import Page from '../../routes/(app)/settings/backups/+page.svelte';
</script>

<Tooltip.Provider>
	<Page />
</Tooltip.Provider>
