<!-- Three SlideOvers for unit tests, in this order in the page; which of them are open comes from the props.
     The tooltips of their buttons need their provider. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import SlideOver from './SlideOver.svelte';

	let { a = false, b = false, c = false }: { a?: boolean; b?: boolean; c?: boolean } = $props();
</script>

<Tooltip.Provider>
	<SlideOver open={a} title="A">панель а</SlideOver>
	<SlideOver open={b} title="B">панель б</SlideOver>
	<SlideOver open={c} title="C">панель в</SlideOver>
</Tooltip.Provider>
