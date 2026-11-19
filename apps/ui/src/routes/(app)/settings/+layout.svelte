<!-- «Настройки» (spec 6.8): subsections on the left, the fields of the open one on the right. In a narrow
     window the subsections are a row above the fields; on a phone first the list, then the subsection
     with «←». -->
<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { display } from '#lib/design/prefs.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { SETTINGS_SECTIONS } from '#lib/settings/sections.js';

	let { children }: { children: Snippet } = $props();

	const ROOT = resolve('settings');
	const atRoot = $derived(page.url.pathname === ROOT);
	const showList = $derived(!display.phone || atRoot);
	const showContent = $derived(!display.phone || !atRoot);
	// A subsection's own pages (the report of an import) keep it marked.
	const current = $derived(
		SETTINGS_SECTIONS.find((s) => {
			const href = resolve(s.href);
			return page.url.pathname === href || page.url.pathname.startsWith(`${href}/`);
		})
	);

	// «Настройки» itself is the list of subsections on a phone; elsewhere it opens the first one, also
	// when a phone's window is widened while the list is shown.
	$effect(() => {
		const first = SETTINGS_SECTIONS[0];
		if (!display.phone && atRoot && first) void goto(resolve(first.href), { replaceState: true });
	});
</script>

<svelte:head><title>{current?.label ?? ru.settings.title}</title></svelte:head>

<div class="settings" class:narrow={display.narrow} class:phone={display.phone}>
	{#if showList}
		<nav class="sections" aria-label={ru.settings.sections}>
			{#if display.phone}<h1 class="heading">{ru.settings.title}</h1>{/if}
			{#each SETTINGS_SECTIONS as s (s.id)}
				{@const Icon = s.icon}
				<a
					class="item"
					class:current={current?.id === s.id}
					href={resolve(s.href)}
					aria-current={current?.id === s.id ? 'page' : undefined}
				>
					<Icon size={16} aria-hidden="true" />
					<span>{s.label}</span>
				</a>
			{/each}
		</nav>
	{/if}
	{#if showContent}
		<div class="content">
			{#if display.phone}
				<a class="back" href={ROOT}>
					<ArrowLeft size={16} aria-hidden="true" />
					<span>{ru.settings.title}</span>
				</a>
			{/if}
			{@render children()}
		</div>
	{/if}
</div>

<style>
	.settings {
		box-sizing: border-box;
		display: grid;
		grid-template-columns: 14rem minmax(0, 1fr);
		align-items: start;
		align-content: start;
		gap: var(--space-4);
		min-height: 100%;
		padding: var(--space-4);
	}
	.settings.narrow {
		grid-template-columns: minmax(0, 1fr);
	}
	.settings.phone {
		padding: var(--space-3);
	}
	.sections {
		display: flex;
		flex-direction: column;
		gap: var(--space-0-5);
	}
	.narrow:not(.phone) .sections {
		flex-flow: row wrap;
	}
	.heading {
		margin: 0 0 var(--space-2);
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.item,
	.back {
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
	.item:hover,
	.back:hover {
		background: var(--surface-3);
		color: var(--text);
	}
	.item.current {
		background: var(--accent-soft);
		color: var(--accent-text);
		font-weight: 600;
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
		max-width: 44rem;
	}
	.back {
		align-self: flex-start;
	}
</style>
