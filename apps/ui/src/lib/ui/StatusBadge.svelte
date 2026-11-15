<script lang="ts">
	import { statusHue } from '#lib/design/status-colors.js';

	let {
		name,
		color,
		size = 'md',
		dot = true
	}: { name: string; color: string; size?: 'sm' | 'md'; dot?: boolean } = $props();
</script>

<span class="badge st-{statusHue(color)} {size}">
	{#if dot}<span class="dot" aria-hidden="true"></span>{/if}
	<span>{name}</span>
</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
		border-radius: var(--radius-full);
		font-weight: 500;
		white-space: nowrap;
	}
	.md {
		height: 1.5rem;
		padding: 0 var(--space-2);
		font-size: var(--text-sm);
	}
	.sm {
		height: 1.25rem;
		padding: 0 var(--space-1-5);
		font-size: var(--text-xs);
	}
	.dot {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: var(--radius-full);
		background: var(--st-dot);
	}
</style>
