<!-- The text of the old MyWork 2 act (plan 7.2): read only, with its line breaks. «Вставить текстом»
     hands the text to the editor, which puts it into the template as a «Текст» block. -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let {
		open = $bindable(false),
		text,
		oninsert
	}: {
		open: boolean;
		text: string;
		oninsert: (text: string) => void;
	} = $props();

	const t = ru.templates;
</script>

<Dialog bind:open title={t.legacyTitle} description={t.legacyHint} size="lg">
	<pre class="text" tabindex="0" aria-label={t.legacyTitle}>{text}</pre>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.close}</Button>
		<Button
			variant="primary"
			onclick={() => {
				oninsert(text);
				open = false;
			}}>{t.legacyInsert}</Button
		>
	{/snippet}
</Dialog>

<style>
	.text {
		margin: 0;
		max-height: 60vh;
		overflow: auto;
		padding: var(--space-3);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		font: inherit;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
	}
</style>
