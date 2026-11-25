<!-- A badge that puts something into a text under the caret: a pill with the insert icon and a label;
     on hover and focus a tooltip shows the code it puts in (monospaced). -->
<script lang="ts">
	import TextCursorInput from '@lucide/svelte/icons/text-cursor-input';
	import { ru } from '#lib/i18n/ru.js';
	import { mergeTriggerProps } from './merge-props.js';
	import Tooltip from './Tooltip.svelte';

	let {
		label,
		code,
		onclick,
		disabled = false,
		...rest
	}: {
		label: string;
		/** What is put in; shown in the tooltip. */
		code: string;
		onclick: () => void;
		disabled?: boolean;
		'aria-expanded'?: boolean;
	} = $props();
</script>

<Tooltip text={code} mono>
	{#snippet children(triggerProps)}
		<button
			{...mergeTriggerProps(triggerProps, { ...rest, onclick })}
			type="button"
			class="chip"
			aria-label={ru.templates.insertBadge(label)}
			{disabled}
		>
			<TextCursorInput size={14} aria-hidden="true" />
			<span>{label}</span>
		</button>
	{/snippet}
</Tooltip>

<style>
	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		box-sizing: border-box;
		min-height: var(--control-sm);
		padding: 0 var(--space-2);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface);
		color: var(--text-2);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}
	.chip:hover:not(:disabled),
	.chip[aria-expanded='true'] {
		background: var(--surface-3);
		color: var(--text);
	}
	.chip:disabled {
		color: var(--text-disabled);
		cursor: not-allowed;
	}
</style>
