<script lang="ts">
	let {
		checked = $bindable(false),
		label,
		hint,
		disabled = false
	}: { checked: boolean; label: string; hint?: string; disabled?: boolean } = $props();

	const uid = $props.id();
</script>

<label class="row" class:disabled>
	<span class="track-wrap">
		<input
			type="checkbox"
			role="switch"
			aria-checked={checked}
			bind:checked
			{disabled}
			aria-describedby={hint ? `${uid}-hint` : undefined}
		/>
		<span class="track" aria-hidden="true"><span class="thumb"></span></span>
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
	.track-wrap {
		position: relative;
		display: inline-flex;
		flex: none;
		width: 2.25rem;
		height: 1.25rem;
		margin-top: 0.125rem;
	}
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: inherit;
	}
	.track {
		width: 100%;
		height: 100%;
		box-sizing: border-box;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface-3);
		transition: background var(--duration-fast);
		pointer-events: none;
	}
	.thumb {
		display: block;
		width: 0.875rem;
		height: 0.875rem;
		margin: 0.1875rem 0 0 0.1875rem;
		border-radius: var(--radius-full);
		background: var(--surface);
		box-shadow: var(--shadow-xs);
		transition: transform var(--duration-fast);
	}
	input:checked + .track {
		background: var(--accent);
		border-color: var(--accent);
	}
	input:checked + .track .thumb {
		transform: translateX(1rem);
	}
	input:disabled + .track {
		opacity: 0.5;
	}
	input:focus-visible + .track {
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
