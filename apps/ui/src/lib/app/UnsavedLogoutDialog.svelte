<!-- «Есть несохранённые правки» on signing out: the edits could not be sent in time. «Сохранить» tries
     again, «Выйти без сохранения» gives them up; closing the window stays signed in. -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let {
		open = $bindable(false),
		busy = false,
		failed = false,
		onsave,
		onleave
	}: {
		open: boolean;
		/** «Сохранить» is trying again. */
		busy?: boolean;
		/** The last «Сохранить» did not succeed. */
		failed?: boolean;
		onsave: () => void;
		onleave: () => void;
	} = $props();
</script>

<Dialog bind:open title={ru.app.unsavedTitle} size="sm" dismissible={!busy}>
	<p class="text">{ru.app.unsavedText}</p>
	{#if failed && !busy}<p class="failed" role="alert">{ru.app.unsavedFailed}</p>{/if}
	{#snippet footer()}
		<Button variant="danger" disabled={busy} onclick={onleave}>{ru.app.unsavedLeave}</Button>
		<Button variant="primary" {busy} onclick={onsave}>{ru.app.unsavedSave}</Button>
	{/snippet}
</Dialog>

<style>
	.text {
		margin: 0;
		color: var(--text-2);
	}
	.failed {
		margin: var(--space-2) 0 0;
		color: var(--danger);
	}
</style>
