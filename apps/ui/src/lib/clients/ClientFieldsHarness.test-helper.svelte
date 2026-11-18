<!-- ClientFields for unit tests: the tooltips of its buttons need their provider, as in the app. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ComponentProps } from 'svelte';
	import ClientFields from './ClientFields.svelte';

	let props: ComponentProps<typeof ClientFields> = $props();
</script>

<Tooltip.Provider>
	<ClientFields {...props} />
</Tooltip.Provider>
