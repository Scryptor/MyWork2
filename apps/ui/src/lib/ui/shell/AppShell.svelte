<script lang="ts" module>
	import { getContext } from 'svelte';
	import type { ShellLayout } from '#lib/design/breakpoints.js';

	const KEY = Symbol('mw-shell-layout');

	/** Current layout of the enclosing AppShell; read it inside the shell's snippets. */
	export function getShellLayout(): () => ShellLayout {
		return getContext<() => ShellLayout>(KEY);
	}
</script>

<script lang="ts">
	import { setContext, type Snippet } from 'svelte';
	import { display } from '#lib/design/prefs.svelte.js';

	let {
		topbar,
		sidebar,
		bottomNav,
		children
	}: { topbar: Snippet; sidebar: Snippet; bottomNav?: Snippet; children: Snippet } = $props();

	const layout = $derived<ShellLayout>(display.value.layout);

	setContext(KEY, () => layout);
</script>

<div class="shell {layout}">
	<div class="top">{@render topbar()}</div>
	{#if layout !== 'phone'}
		<div class="side">{@render sidebar()}</div>
	{/if}
	<!-- tabindex: focus lands here when the element that opened a dialog is gone. -->
	<main class="main" tabindex="-1">{@render children()}</main>
	{#if layout === 'phone' && bottomNav}
		<div class="bottom">{@render bottomNav()}</div>
	{/if}
</div>

<style>
	.shell {
		display: grid;
		grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
		grid-template-rows: var(--topbar-height) minmax(0, 1fr);
		grid-template-areas:
			'top top'
			'side main';
		height: 100%;
		background: var(--bg);
		color: var(--text);
	}
	.shell.rail {
		grid-template-columns: var(--rail-width) minmax(0, 1fr);
	}
	.shell.phone {
		grid-template-columns: minmax(0, 1fr);
		grid-template-rows: var(--topbar-height) minmax(0, 1fr) auto;
		grid-template-areas:
			'top'
			'main'
			'bottom';
	}
	.top {
		grid-area: top;
		position: relative;
		z-index: var(--z-shell);
	}
	.side {
		grid-area: side;
		min-height: 0;
		overflow-y: auto;
	}
	/* Positioned: the containing block of what is absolutely positioned inside (the visually hidden words
	   of list cards), so it scrolls with the content instead of making the whole page scroll. */
	.main {
		position: relative;
		grid-area: main;
		min-width: 0;
		min-height: 0;
		overflow: auto;
	}
	.bottom {
		grid-area: bottom;
		position: relative;
		z-index: var(--z-shell);
	}
</style>
