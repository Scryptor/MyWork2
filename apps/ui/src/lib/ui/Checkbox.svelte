<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import Minus from '@lucide/svelte/icons/minus';

	let {
		checked = $bindable(false),
		label,
		hint,
		disabled = false,
		indeterminate = false
	}: {
		checked: boolean;
		label: string;
		hint?: string;
		disabled?: boolean;
		indeterminate?: boolean;
	} = $props();

	const uid = $props.id();
	let el = $state<HTMLInputElement | null>(null);
	$effect(() => {
		if (el) el.indeterminate = indeterminate;
	});
</script>

<label class="row" class:disabled>
	<span class="box">
		<input
			type="checkbox"
			bind:this={el}
			bind:checked
			{disabled}
			aria-describedby={hint ? `${uid}-hint` : undefined}
		/>
		<span class="mark" aria-hidden="true">
			{#if indeterminate}<Minus size={14} />{:else if checked}<Check size={14} />{/if}
		</span>
	</span>
	<span class="text">
		<span>{label}</span>
		{#if hint}<span class="hint" id="{uid}-hint">{hint}</span>{/if}
	</span>
</label>

<style>
	.row {
		display: inline-flex;
		align-items: flex-start;
		gap: var(--space-2);
		cursor: pointer;
		min-height: var(--control-sm);
	}
	.row.disabled {
		cursor: not-allowed;
		color: var(--text-disabled);
	}
	.box {
		position: relative;
		display: inline-flex;
		flex: none;
		width: 1.125rem;
		height: 1.125rem;
		margin-top: 0.1875rem;
	}
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: inherit;
	}
	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		box-sizing: border-box;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-xs);
		background: var(--surface);
		color: var(--on-accent);
		pointer-events: none;
	}
	input:checked + .mark,
	input:indeterminate + .mark {
		background: var(--accent);
		border-color: var(--accent);
	}
	input:disabled + .mark {
		background: var(--surface-3);
		border-color: var(--border-strong);
	}
	input:focus-visible + .mark {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
	.text {
		display: flex;
		flex-direction: column;
	}
	.hint {
		font-size: var(--text-sm);
		color: var(--text-3);
	}
</style>
