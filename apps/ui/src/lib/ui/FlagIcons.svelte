<script lang="ts">
	import Package from '@lucide/svelte/icons/package';
	import Phone from '@lucide/svelte/icons/phone';
	import { ru } from '#lib/i18n/ru.js';
	import Tooltip from './Tooltip.svelte';
	import VisuallyHidden from './VisuallyHidden.svelte';

	let {
		needCall,
		needParts,
		blacklisted = false
	}: { needCall: boolean; needParts: boolean; blacklisted?: boolean } = $props();
</script>

<span class="flags">
	{#if needCall}
		<Tooltip text={ru.flags.needCall}>
			{#snippet children(p)}
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<span {...p} class="flag" tabindex="0">
					<Phone size={14} aria-hidden="true" />
					<VisuallyHidden>{ru.flags.needCall}</VisuallyHidden>
				</span>
			{/snippet}
		</Tooltip>
	{/if}
	{#if needParts}
		<Tooltip text={ru.flags.needParts}>
			{#snippet children(p)}
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<span {...p} class="flag" tabindex="0">
					<Package size={14} aria-hidden="true" />
					<VisuallyHidden>{ru.flags.needParts}</VisuallyHidden>
				</span>
			{/snippet}
		</Tooltip>
	{/if}
	{#if blacklisted}
		<Tooltip text={ru.flags.blacklisted}>
			{#snippet children(p)}
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<span {...p} class="flag" tabindex="0">
					<span class="dot" aria-hidden="true"></span>
					<VisuallyHidden>{ru.flags.blacklisted}</VisuallyHidden>
				</span>
			{/snippet}
		</Tooltip>
	{/if}
</span>

<style>
	.flags {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
	}
	.flag {
		position: relative;
		display: inline-flex;
		color: var(--text-2);
	}
	.dot {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: var(--radius-full);
		background: var(--danger);
	}
</style>
