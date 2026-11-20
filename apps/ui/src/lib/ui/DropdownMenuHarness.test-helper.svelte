<script lang="ts">
	import DropdownMenu, { type MenuItem } from './DropdownMenu.svelte';

	let { items }: { items: MenuItem[] } = $props();
</script>

<DropdownMenu {items} label="Ещё статусы">
	{#snippet trigger(props)}<button type="button" {...props}>Ещё</button>{/snippet}
</DropdownMenu>
