<!-- A row of a list on a phone: number and status, what it is, who and when, flags. -->
<script lang="ts">
	import Package from '@lucide/svelte/icons/package';
	import Phone from '@lucide/svelte/icons/phone';
	import { ru } from '#lib/i18n/ru.js';
	import StatusBadge from './StatusBadge.svelte';
	import VisuallyHidden from './VisuallyHidden.svelte';

	let {
		number,
		title,
		subtitle,
		meta,
		status,
		flags,
		active = false,
		onclick
	}: {
		number: string;
		title: string;
		subtitle?: string;
		meta?: string;
		status?: { name: string; color: string };
		flags?: { needCall: boolean; needParts: boolean };
		active?: boolean;
		onclick?: () => void;
	} = $props();

	const hasFlags = $derived(!!flags && (flags.needCall || flags.needParts));
	const line = $derived([subtitle, meta].filter((s) => !!s).join(' · '));
</script>

{#snippet content()}
	<span class="top">
		<span class="number">{number}</span>
		{#if status}<StatusBadge name={status.name} color={status.color} size="sm" />{/if}
	</span>
	<!-- Cut to one line like a table cell; the whole text is in the hint. -->
	<span class="title" {title}>{title}</span>
	{#if line || hasFlags}
		<span class="bottom">
			<span class="meta">{line}</span>
			{#if flags && hasFlags}
				<!-- Icons only inside the card: it is one button, so no tooltips; the words are for screen readers. -->
				<span class="flags">
					{#if flags.needCall}
						<Phone size={16} aria-hidden="true" /><VisuallyHidden
							>{ru.flags.needCall}</VisuallyHidden
						>
					{/if}
					{#if flags.needParts}
						<Package size={16} aria-hidden="true" /><VisuallyHidden
							>{ru.flags.needParts}</VisuallyHidden
						>
					{/if}
				</span>
			{/if}
		</span>
	{/if}
{/snippet}

{#if onclick}
	<button
		type="button"
		class="card"
		class:active
		aria-current={active ? 'true' : undefined}
		{onclick}
	>
		{@render content()}
	</button>
{:else}
	<div class="card" class:active aria-current={active ? 'true' : undefined}>
		{@render content()}
	</div>
{/if}

<style>
	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		width: 100%;
		min-height: var(--card-min-height);
		box-sizing: border-box;
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow-xs);
		font: inherit;
		font-size: var(--text-md);
		line-height: var(--leading-tight);
		text-align: left;
	}
	button.card {
		cursor: pointer;
	}
	@media (hover: hover) {
		button.card:hover {
			background: var(--surface-3);
		}
	}
	.card.active {
		border-color: var(--accent);
		background: var(--accent-soft);
		box-shadow: inset 3px 0 0 var(--accent);
	}
	.top,
	.bottom {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-width: 0;
	}
	.number {
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.title,
	.meta {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.title {
		font-weight: 500;
	}
	/* Touch has no hover, so the `title` hint is out of reach: the whole title is shown, wrapped. */
	@media (pointer: coarse) {
		.title {
			overflow: visible;
			text-overflow: clip;
			white-space: normal;
			overflow-wrap: anywhere;
		}
	}
	.meta {
		color: var(--text-3);
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
	}
	.flags {
		display: inline-flex;
		flex: none;
		gap: var(--space-1-5);
		color: var(--text-2);
	}
</style>
