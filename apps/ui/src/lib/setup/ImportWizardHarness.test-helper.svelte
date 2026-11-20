<!-- The wizard as the program has it: under the provider of tooltips (the icons of «Что это?» need it). -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ComponentProps } from 'svelte';
	import ImportWizard from './ImportWizard.svelte';

	let props: ComponentProps<typeof ImportWizard> = $props();
</script>

<Tooltip.Provider>
	<ImportWizard {...props} />
</Tooltip.Provider>
