<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import VisuallyHidden from './VisuallyHidden.svelte';

	let { size = 'md', label = ru.common.loading }: { size?: 'sm' | 'md'; label?: string } = $props();
</script>

<span class="spinner {size}" role="status">
	<span class="ring" aria-hidden="true"></span>
	<VisuallyHidden>{label}</VisuallyHidden>
</span>

<style>
	.spinner {
		display: inline-flex;
		position: relative;
	}
	.ring {
		box-sizing: border-box;
		border-radius: var(--radius-full);
		border: 2px solid currentColor;
		border-right-color: transparent;
		animation: spin 700ms linear infinite;
	}
	.sm .ring {
		width: 0.875rem;
		height: 0.875rem;
	}
	.md .ring {
		width: 1.125rem;
		height: 1.125rem;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.ring {
			animation-duration: 2.4s;
		}
	}
</style>
