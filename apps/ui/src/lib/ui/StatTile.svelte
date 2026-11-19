<!-- A total in big digits with what it counts (the report of a transfer). -->
<script lang="ts">
	let {
		label,
		value,
		hint,
		tone
	}: { label: string; value: string; hint?: string; tone?: 'warning' } = $props();
</script>

<div class="tile" class:warning={tone === 'warning'}>
	<span class="value num">{value}</span>
	<span class="label">{label}</span>
	{#if hint}<span class="hint">{hint}</span>{/if}
</div>

<style>
	.tile {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
		box-sizing: border-box;
		height: 100%;
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
	}
	.value {
		font-size: var(--text-2xl);
		font-weight: 600;
		line-height: 1.2;
	}
	.label {
		color: var(--text-2);
		font-size: var(--text-md);
	}
	.hint {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.warning {
		border-color: transparent;
		background: var(--warning-soft);
	}
	.warning .value,
	.warning .label {
		color: var(--warning-text);
	}
</style>
