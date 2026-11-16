<script lang="ts">
	import '@fontsource-variable/inter';
	import '#lib/design/tokens.css';
	import '#lib/design/base.css';
	import { Tooltip } from 'bits-ui';
	import { prefs } from '#lib/design/prefs.svelte.js';
	import Toaster from '#lib/ui/toast/Toaster.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	$effect(() => prefs.init());
</script>

<Tooltip.Provider delayDuration={400}>
	{@render children()}
	<Toaster />
</Tooltip.Provider>
