<script lang="ts">
	/* eslint-disable svelte/no-navigation-without-resolve -- hrefs come ready from the caller */
	import type { Component } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';
	import type { NavItem } from './nav.js';

	let {
		items,
		active,
		primary,
		onnavigate
	}: {
		items: NavItem[];
		active: string;
		primary?: { label: string; icon: Component; onclick: () => void };
		onnavigate?: (id: string) => void;
	} = $props();

	const PrimaryIcon = $derived(primary?.icon);
</script>

<nav class="bottom" aria-label={ru.shell.bottomNav}>
	{#each items as item (item.id)}
		{@const Icon = item.icon}
		<a
			class="item"
			class:current={item.id === active}
			href={item.href ?? '#'}
			aria-current={item.id === active ? 'page' : undefined}
			onclick={() => onnavigate?.(item.id)}
		>
			<span class="glyph">
				<Icon size={20} aria-hidden="true" />
				{#if item.count}
					<span class="badge num" class:attention={item.attention}>{item.count}</span>
				{/if}
			</span>
			<span class="label">{item.label}</span>
		</a>
	{/each}
	{#if primary && PrimaryIcon}
		<button type="button" class="item primary" onclick={primary.onclick}>
			<span class="glyph accent"><PrimaryIcon size={20} aria-hidden="true" /></span>
			<span class="label">{primary.label}</span>
		</button>
	{/if}
</nav>

<style>
	.bottom {
		display: flex;
		box-sizing: border-box;
		min-height: var(--bottom-nav-height);
		padding-bottom: env(safe-area-inset-bottom, 0px);
		border-top: 1px solid var(--border);
		background: var(--surface);
	}
	.item {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-0-5);
		min-width: 0;
		min-height: 2.75rem;
		padding: var(--space-1) 0;
		border: 0;
		background: transparent;
		color: var(--text-2);
		font: inherit;
		font-size: var(--text-xs);
		text-decoration: none;
		cursor: pointer;
	}
	.item.current {
		color: var(--accent-text);
		font-weight: 600;
	}
	.glyph {
		position: relative;
		display: inline-flex;
	}
	.glyph.accent {
		align-items: center;
		justify-content: center;
		width: var(--control-lg);
		height: var(--control-md);
		border-radius: var(--radius-full);
		background: var(--accent);
		color: var(--on-accent);
	}
	.label {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.badge {
		position: absolute;
		top: calc(-1 * var(--space-1));
		left: 100%;
		min-width: 1rem;
		padding: 0 var(--space-1);
		border-radius: var(--radius-full);
		background: var(--surface-3);
		color: var(--text-2);
		font-size: var(--text-xs);
		line-height: 1rem;
		text-align: center;
	}
	.badge.attention {
		background: var(--warning-soft);
		color: var(--warning-text);
	}
</style>
