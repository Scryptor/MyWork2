<!-- Several orders become issued at once (spec 6.1): stamp today's issue date or leave it empty, as for
     orders given back long ago. Ctrl/⌘+Enter is «Сейчас». -->
<script lang="ts">
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let {
		open = $bindable(false),
		status,
		count,
		onchoose
	}: {
		open: boolean;
		/** The issued status being set. */
		status: string;
		count: number;
		/** `true`: «Сейчас», the date is stamped where there is none; `false`: «Не ставить». */
		onchoose: (stamp: boolean) => void;
	} = $props();

	function choose(stamp: boolean) {
		open = false;
		onchoose(stamp);
	}

	$effect(() => {
		if (open) return submitShortcut(() => choose(true));
	});
</script>

<Dialog bind:open title={ru.bulk.issuedDateTitle} size="sm">
	<p class="text">{ru.bulk.issuedDateText(status, countLabel(count, ru.bulk.forms))}</p>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button onclick={() => choose(false)}>{ru.bulk.noStamp}</Button>
		<Button variant="primary" shortcut="Mod+Enter" onclick={() => choose(true)}
			>{ru.bulk.stampNow}</Button
		>
	{/snippet}
</Dialog>

<style>
	.text {
		margin: 0;
		color: var(--text-2);
	}
</style>
