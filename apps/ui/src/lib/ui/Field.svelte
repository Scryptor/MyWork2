<script lang="ts">
	import type { Snippet } from 'svelte';
	import './field.css';
	import HelpHint from './HelpHint.svelte';

	let {
		label,
		hint,
		help,
		error,
		required = false,
		id,
		children
	}: {
		label?: string;
		hint?: string;
		/** «Что это?»: an icon right of the label that opens the explanation; needs a `label`. */
		help?: string | readonly string[];
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
		<!-- The icon is outside <label>: pressing it must not move focus into the field. -->
		<div class="head">
			<label for={id}
				>{label}{#if required}&nbsp;<span class="req">*</span>{/if}</label
			>
			{#if help}<span class="help"><HelpHint title={label} text={help} /></span>{/if}
		</div>
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
	.head {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
	}
	/* The icon does not make the line of its label taller: fields side by side stay level, with a hint
	   or without. On a touch screen the button is still 44px: it reaches over the gap above and below. */
	.help {
		display: inline-flex;
		margin-block: calc((var(--text-sm) * var(--leading-normal) - var(--control-sm)) / 2);
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
