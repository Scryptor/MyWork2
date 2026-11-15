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
			<span class="flag"
				><Phone size={14} aria-hidden="true" /><VisuallyHidden>{ru.flags.needCall}</VisuallyHidden
				></span
			>
		</Tooltip>
	{/if}
	{#if needParts}
		<Tooltip text={ru.flags.needParts}>
			<span class="flag"
				><Package size={14} aria-hidden="true" /><VisuallyHidden
					>{ru.flags.needParts}</VisuallyHidden
				></span
			>
		</Tooltip>
	{/if}
	{#if blacklisted}
		<Tooltip text={ru.flags.blacklisted}>
			<span class="flag"
				><span class="dot" aria-hidden="true"></span><VisuallyHidden
					>{ru.flags.blacklisted}</VisuallyHidden
				></span
			>
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
