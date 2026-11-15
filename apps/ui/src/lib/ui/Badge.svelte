<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		tone = 'neutral',
		children
	}: {
		tone?: 'neutral' | 'accent' | 'danger' | 'warning' | 'success' | 'info';
		children: Snippet;
	} = $props();
</script>

<span class="badge {tone}">{@render children()}</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		height: 1.25rem;
		padding: 0 var(--space-2);
		border-radius: var(--radius-full);
		font-size: var(--text-xs);
		font-weight: 500;
		white-space: nowrap;
	}
	.neutral {
		background: var(--surface-3);
		color: var(--text-2);
	}
	.accent {
		background: var(--accent-soft);
		color: var(--accent-text);
	}
	.danger {
		background: var(--danger-soft);
		color: var(--danger-text);
	}
	.warning {
		background: var(--warning-soft);
		color: var(--warning-text);
	}
	.success {
		background: var(--success-soft);
		color: var(--success-text);
	}
	.info {
		background: var(--info-soft);
		color: var(--info-text);
	}
</style>
