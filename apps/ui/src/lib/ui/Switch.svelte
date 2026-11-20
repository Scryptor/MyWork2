<script lang="ts">
	import HelpHint from './HelpHint.svelte';

	let {
		checked = $bindable(false),
		label,
		hint,
		help,
		disabled = false
	}: {
		checked: boolean;
		label: string;
		hint?: string;
		/** «Что это?»: an icon right of the switch that opens the explanation. */
		help?: string | readonly string[];
		disabled?: boolean;
	} = $props();

	const uid = $props.id();
</script>

<div class="wrap">
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
	{#if help}<HelpHint title={label} text={help} />{/if}
</div>

<style>
	.wrap {
		display: inline-flex;
		align-items: flex-start;
		gap: var(--space-1);
	}
	.row {
		display: inline-flex;
		align-items: flex-start;
		gap: var(--space-2);
		cursor: pointer;
		min-height: var(--control-sm);
	}
	@media (pointer: coarse) {
		.row {
			min-height: var(--control-md);
			align-items: center;
		}
	}
	.row.disabled {
		cursor: not-allowed;
		color: var(--text-disabled);
	}
	/* The thumb is centred by the flex track; the gap on every side is the same, border included. */
	.track-wrap {
		--switch-width: 2.25rem;
		--switch-height: 1.25rem;
		--switch-thumb: 0.875rem;
		--switch-gap: calc((var(--switch-height) - var(--switch-thumb)) / 2);
		position: relative;
		display: inline-flex;
		flex: none;
		width: var(--switch-width);
		height: var(--switch-height);
		margin-top: 0.125rem;
	}
	@media (pointer: coarse) {
		.track-wrap {
			margin-top: 0;
		}
	}
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: inherit;
	}
	.track {
		display: flex;
		align-items: center;
		width: 100%;
		height: 100%;
		box-sizing: border-box;
		padding: 0 calc(var(--switch-gap) - 1px);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface-3);
		transition: background var(--duration-fast);
		pointer-events: none;
	}
	.thumb {
		display: block;
		flex: none;
		width: var(--switch-thumb);
		height: var(--switch-thumb);
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
		transform: translateX(calc(var(--switch-width) - var(--switch-thumb) - 2 * var(--switch-gap)));
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
