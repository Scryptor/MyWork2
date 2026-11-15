<script lang="ts">
	import '@fontsource-variable/inter';
	import '#lib/design/tokens.css';
	import '#lib/design/base.css';
	import { prefs } from '#lib/design/prefs.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	$effect(() => prefs.init());
</script>

{@render children()}
