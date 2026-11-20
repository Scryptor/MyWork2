<!-- The form as the program has it: under the provider of tooltips (the icons of «Что это?» need it). -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ComponentProps } from 'svelte';
	import SetupForm from './SetupForm.svelte';

	let props: ComponentProps<typeof SetupForm> = $props();
</script>

<Tooltip.Provider>
	<SetupForm {...props} />
</Tooltip.Provider>
