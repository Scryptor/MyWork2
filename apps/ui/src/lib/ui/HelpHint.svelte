<!-- «Что это?»: an icon next to a label; a click opens a small window with the explanation.
     The window is a layer, so one-key hotkeys do not fire while it is open; Ctrl/⌘+Enter closes it
     instead of submitting the dialog underneath. -->
<script lang="ts">
	import CircleHelp from '@lucide/svelte/icons/circle-help';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from './Button.svelte';
	import Dialog from './Dialog.svelte';
	import IconButton from './IconButton.svelte';

	let { title, text }: { title: string; text: string | readonly string[] } = $props();

	let open = $state(false);
	const paragraphs = $derived(typeof text === 'string' ? [text] : text);
	// The newest submit shortcut wins: while the hint is open, Ctrl/⌘+Enter is «Понятно».
	$effect(() => {
		if (open) return submitShortcut(() => (open = false));
	});
</script>

<IconButton label={ru.help.open(title)} icon={CircleHelp} size="sm" onclick={() => (open = true)} />

<Dialog bind:open {title} size="sm">
	<div class="text">
		{#each paragraphs as p, i (i)}<p>{p}</p>{/each}
	</div>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.understood}</Button>
	{/snippet}
</Dialog>

<style>
	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	p {
		margin: 0;
	}
</style>
