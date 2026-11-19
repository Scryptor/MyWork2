<!-- How far a long job has got (the stages of a transfer): a bar with its value for screen readers. -->
<script lang="ts">
	let { value, label, max = 100 }: { value: number; label: string; max?: number } = $props();

	const share = $derived(max > 0 ? Math.max(0, Math.min(1, value / max)) : 0);
</script>

<div
	class="bar"
	role="progressbar"
	aria-label={label}
	aria-valuemin={0}
	aria-valuemax={max}
	aria-valuenow={Math.round(value)}
>
	<div class="fill" style:width="{share * 100}%"></div>
</div>

<style>
	.bar {
		overflow: hidden;
		height: var(--space-2);
		border-radius: var(--radius-full);
		background: var(--surface-3);
	}
	.fill {
		height: 100%;
		border-radius: inherit;
		background: var(--accent);
		transition: width var(--duration-normal) var(--ease-out);
	}
</style>
