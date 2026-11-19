<!-- What happened to the changes of a settings screen: saving, saved, or not saved with «Повторить». -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import type { SettingsEditor } from './settings-editor.svelte.js';

	let { editor }: { editor: SettingsEditor } = $props();

	const t = ru.save;
</script>

<div class="note" role="status">
	{#if editor.error}
		<span class="failed">{t.failed}: {editor.error.message}</span>
		<Button size="sm" variant="ghost" onclick={() => editor.retry()}>{ru.common.retry}</Button>
	{:else if editor.fieldsFailed}
		<span class="failed">{t.checkFields}</span>
	{:else if editor.state === 'saving'}
		<span>{t.saving}</span>
	{:else if editor.state === 'saved'}
		<span>{t.saved}</span>
	{/if}
</div>

<style>
	.note {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--control-sm);
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.failed {
		color: var(--danger-text);
	}
</style>
