<script lang="ts">
	import type { Snippet } from 'svelte';
	import './field.css';

	let {
		label,
		hint,
		error,
		required = false,
		id,
		children
	}: {
		label?: string;
		hint?: string;
		error?: string;
		required?: boolean;
		id: string;
		children: Snippet<[{ id: string; describedBy: string | undefined; invalid: boolean }]>;
	} = $props();

	const describedBy = $derived(
		[hint ? `${id}-hint` : null, error ? `${id}-error` : null].filter(Boolean).join(' ') ||
			undefined
	);
</script>

<div class="field">
	{#if label}
		<label for={id}
			>{label}{#if required}&nbsp;<span class="req">*</span>{/if}</label
		>
	{/if}
	{@render children({ id, describedBy, invalid: Boolean(error) })}
	{#if hint}<div class="hint" id="{id}-hint">{hint}</div>{/if}
	{#if error}<div class="error" id="{id}-error">{error}</div>{/if}
</div>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}
	label {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--text-2);
	}
	.req {
		color: var(--danger-text);
	}
	.hint {
		font-size: var(--text-sm);
		color: var(--text-3);
	}
	.error {
		font-size: var(--text-sm);
		color: var(--danger-text);
	}
</style>
