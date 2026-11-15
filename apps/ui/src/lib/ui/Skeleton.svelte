<script lang="ts">
	let {
		width = '100%',
		height = '1rem',
		radius = 'sm'
	}: { width?: string; height?: string; radius?: 'sm' | 'md' | 'full' } = $props();
</script>

<span
	class="skeleton"
	aria-hidden="true"
	style:width
	style:height
	style:border-radius={`var(--radius-${radius})`}
></span>

<style>
	.skeleton {
		display: block;
		background: linear-gradient(90deg, var(--surface-3), var(--surface-2), var(--surface-3));
		background-size: 200% 100%;
		animation: shimmer 1.4s ease-in-out infinite;
	}
	@keyframes shimmer {
		to {
			background-position: -200% 0;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.skeleton {
			animation: none;
		}
	}
</style>
