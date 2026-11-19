<!-- «Сохраняю…», «✓ Сохранено» or «Не сохранено · Повторить» of a record's editor (spec 6.2: an order, a
     client), after the other parts of a meta line: each state starts with its own «·». A save no retry
     can help (the record deleted, no right to edit) says why and offers «Отбросить мои правки» instead. -->
<script lang="ts">
	import type { SaveStatus } from '#lib/app/entity-editor.svelte.js';
	import { ru } from '#lib/i18n/ru.js';

	let { editor }: { editor: SaveStatus } = $props();

	const saveState = $derived(editor.saveState);
	const lost = $derived(editor.lost);
</script>

<span class="save" aria-live="polite" title={editor.autosave.error?.message}>
	{#if lost}
		<!-- Also while leaving a field sends it again: the button does not slip away from under the click. -->
		<span aria-hidden="true">·</span>
		<span class="failed">{ru.save.failed}: {lost}</span>
		<span aria-hidden="true">·</span>
		<button type="button" class="retry" onclick={() => editor.drop()}>{ru.save.drop}</button>
	{:else if saveState === 'saving'}
		<span aria-hidden="true">·</span> <span>{ru.save.saving}</span>
	{:else if saveState === 'saved'}
		<span aria-hidden="true">·</span> <span class="saved">✓ {ru.save.saved}</span>
	{:else if saveState === 'error' || saveState === 'conflict'}
		<span aria-hidden="true">·</span> <span class="failed">{ru.save.failed}</span>
		{#if saveState === 'error'}
			<span aria-hidden="true">·</span>
			<button type="button" class="retry" onclick={() => editor.autosave.retry()}
				>{ru.common.retry}</button
			>
		{/if}
	{/if}
</span>

<style>
	.save {
		display: contents;
	}
	.saved {
		color: var(--success-text);
	}
	.failed {
		color: var(--danger-text);
	}
	.retry {
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--accent);
		font: inherit;
		cursor: pointer;
	}
	.retry:hover {
		text-decoration: underline;
	}
	@media (pointer: coarse) {
		.retry {
			min-height: var(--control-md);
		}
	}
</style>
