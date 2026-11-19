<!-- A key or a shortcut. As a hint next to a control (`hint`, by default) it goes on a touch screen: there
     are no keys to press there, and the room is better left to the label. The list of the shortcuts
     (`hint={false}`) shows the keys everywhere. -->
<script lang="ts">
	import { formatShortcut } from '#lib/design/hotkeys.svelte.js';

	let { shortcut, hint = true }: { shortcut: string; hint?: boolean } = $props();
</script>

<kbd class:hint>{formatShortcut(shortcut)}</kbd>

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
	@media (pointer: coarse) {
		.hint {
			display: none;
		}
	}
</style>
