<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import InsertChip from './InsertChip.svelte';

	let { label, code, onclick }: { label: string; code: string; onclick: () => void } = $props();
</script>

<Tooltip.Provider>
	<InsertChip {label} {code} {onclick} />
</Tooltip.Provider>
