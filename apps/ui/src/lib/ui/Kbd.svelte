<script lang="ts">
	import { formatShortcut } from '#lib/design/hotkeys.svelte.js';

	let { shortcut }: { shortcut: string } = $props();
</script>

<kbd>{formatShortcut(shortcut)}</kbd>

<style>
	kbd {
		font-family: var(--font-sans);
		font-size: var(--text-xs);
		line-height: 1;
		padding: var(--space-0-5) var(--space-1-5);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-xs);
		background: var(--surface-2);
		color: var(--text-2);
	}
</style>
