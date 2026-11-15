<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		tone,
		title,
		children,
		actions
	}: {
		tone: 'info' | 'warning' | 'danger' | 'success';
		title?: string;
		children: Snippet;
		actions?: Snippet;
	} = $props();
</script>

<div class="callout {tone}" role={tone === 'danger' ? 'alert' : 'status'}>
	<div class="body">
		{#if title}<p class="title">{title}</p>{/if}
		<div>{@render children()}</div>
	</div>
	{#if actions}<div class="actions">{@render actions()}</div>{/if}
</div>

<style>
	.callout {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		border-inline-start: 3px solid currentColor;
		font-size: var(--text-md);
	}
	.title {
		margin: 0 0 var(--space-0-5);
		font-weight: 600;
	}
	.actions {
		display: flex;
		gap: var(--space-2);
	}
	.info {
		background: var(--info-soft);
		color: var(--info-text);
	}
	.warning {
		background: var(--warning-soft);
		color: var(--warning-text);
	}
	.danger {
		background: var(--danger-soft);
		color: var(--danger-text);
	}
	.success {
		background: var(--success-soft);
		color: var(--success-text);
	}
</style>
