<script lang="ts">
	/* eslint-disable svelte/no-navigation-without-resolve -- hrefs come ready from the caller */
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';
	import IconButton from '../IconButton.svelte';
	import Tooltip from '../Tooltip.svelte';
	import { getShellLayout } from './AppShell.svelte';
	import type { NavSection } from './nav.js';

	let {
		sections,
		active,
		onnavigate,
		footer
	}: {
		sections: NavSection[];
		active: string;
		onnavigate?: (id: string) => void;
		footer?: Snippet;
	} = $props();

	const layout = getShellLayout();
	const rail = $derived(layout() === 'rail');
</script>

<nav class="sidebar" class:rail aria-label={ru.shell.nav}>
	{#each sections as section, i (i)}
		<div class="section" class:bottom={section.bottom}>
			{#if section.title && !rail}
				<div class="label-caps title">{section.title}</div>
			{:else if i > 0 && rail}
				<hr class="sep" />
			{/if}
			{#each section.items as item (item.id)}
				{@const Icon = item.icon}
				{#snippet body()}
					<Icon size={16} aria-hidden="true" />
					{#if !rail}
						<span class="label">{item.label}</span>
						{#if item.count !== undefined}
							<span class="count num" class:attention={item.attention}>{item.count}</span>
						{/if}
					{:else if item.count !== undefined && item.count > 0}
						<span class="dot" class:attention={item.attention} aria-hidden="true"></span>
					{/if}
				{/snippet}
				{#if rail}
					<Tooltip text={item.label} side="right">
						{#snippet children(props)}
							<a
								{...props}
								class="item"
								class:current={item.id === active}
								href={item.href ?? '#'}
								aria-label={item.count ? `${item.label}: ${item.count}` : item.label}
								aria-current={item.id === active ? 'page' : undefined}
								onclick={() => onnavigate?.(item.id)}>{@render body()}</a
							>
						{/snippet}
					</Tooltip>
				{:else}
					{#snippet link()}
						<a
							class="item"
							class:current={item.id === active}
							href={item.href ?? '#'}
							aria-current={item.id === active ? 'page' : undefined}
							onclick={() => onnavigate?.(item.id)}>{@render body()}</a
						>
					{/snippet}
					{#if item.remove}
						<div class="removable">
							{@render link()}
							<span class="remove">
								<IconButton
									label={item.remove.label}
									icon={X}
									size="sm"
									onclick={item.remove.onclick}
								/>
							</span>
						</div>
					{:else}
						{@render link()}
					{/if}
				{/if}
			{/each}
		</div>
	{/each}
	{#if footer}<div class="footer">{@render footer()}</div>{/if}
</nav>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		box-sizing: border-box;
		width: var(--sidebar-width);
		height: 100%;
		padding: var(--space-3) var(--space-2);
		border-right: 1px solid var(--border);
		background: var(--surface-2);
		overflow-y: auto;
	}
	.sidebar.rail {
		width: var(--rail-width);
		align-items: center;
		padding-inline: var(--space-2);
	}
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
	}
	.section.bottom {
		margin-top: auto;
	}
	.rail .section {
		align-items: center;
	}
	.title {
		padding: var(--space-1) var(--space-2);
	}
	.sep {
		width: 1.5rem;
		margin: var(--space-1) 0;
		border: 0;
		border-top: 1px solid var(--border);
	}
	.item {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		min-height: var(--control-md);
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
		color: var(--text-2);
		font-size: var(--text-md);
		text-decoration: none;
	}
	.rail .item {
		justify-content: center;
		width: var(--control-lg);
		height: var(--control-lg);
		padding: 0;
	}
	.item:hover {
		background: var(--surface-3);
		color: var(--text);
	}
	.item.current {
		background: var(--accent-soft);
		color: var(--accent-text);
		font-weight: 600;
	}
	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.count {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.count.attention {
		color: var(--warning-text);
		font-weight: 600;
	}
	.dot {
		position: absolute;
		top: var(--space-1-5);
		right: var(--space-1-5);
		width: 0.5rem;
		height: 0.5rem;
		border-radius: var(--radius-full);
		background: var(--text-3);
	}
	.dot.attention {
		background: var(--warning-text);
	}
	.removable {
		position: relative;
		display: flex;
		align-items: center;
	}
	.removable .item {
		flex: 1;
		min-width: 0;
		padding-right: calc(var(--control-sm) + var(--space-1));
	}
	.remove {
		position: absolute;
		right: var(--space-0-5);
		display: inline-flex;
		opacity: 0;
	}
	.removable:hover .remove,
	.removable:focus-within .remove {
		opacity: 1;
	}
	.footer {
		margin-top: auto;
	}
	@media (pointer: coarse) {
		.item {
			min-height: 2.75rem;
		}
		.remove {
			opacity: 1;
		}
		.removable .item {
			padding-right: 2.75rem;
		}
	}
</style>
