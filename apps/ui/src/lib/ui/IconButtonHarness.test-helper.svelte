<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Component } from 'svelte';
	import IconButton from './IconButton.svelte';

	let { label, icon }: { label: string; icon: Component } = $props();
</script>

<Tooltip.Provider>
	<IconButton {label} {icon} />
</Tooltip.Provider>
