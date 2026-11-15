<script lang="ts">
	let { items }: { items: { label: string; value: string; muted?: boolean }[] } = $props();
</script>

<dl class="kv">
	{#each items as item (item.label)}
		<dt>{item.label}</dt>
		<dd class:muted={item.muted}>{item.value}</dd>
	{/each}
</dl>

<style>
	.kv {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: var(--space-1-5) var(--space-4);
		margin: 0;
		font-size: var(--text-md);
	}
	dt {
		color: var(--text-3);
	}
	dd {
		margin: 0;
		color: var(--text);
	}
	dd.muted {
		color: var(--text-3);
	}
</style>
