<!-- A field changed here and on another computer (spec 6.2): what the other side has and the choice
     «Оставить моё» / «Взять новое». Orders, clients and stock positions show it under the disputed field. -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import Button from './Button.svelte';

	let {
		label,
		theirs,
		onkeep,
		ontake
	}: {
		/** The field's name, for the group's accessible name. */
		label: string;
		/** The value on the server now, as text. */
		theirs: string;
		onkeep?: () => void;
		ontake?: () => void;
	} = $props();

	const o = ru.order;
</script>

<div class="dispute" role="group" aria-label={o.conflictField(label)}>
	<p class="theirs"><span class="muted">{o.theirs}:</span> {theirs}</p>
	<div class="choices">
		<Button size="sm" onclick={() => onkeep?.()}>{o.keepMine}</Button>
		<Button size="sm" onclick={() => ontake?.()}>{o.takeNew}</Button>
	</div>
</div>

<style>
	.dispute {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--warning-soft);
		color: var(--warning-text);
		font-size: var(--text-sm);
	}
	.theirs {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.muted {
		color: var(--text-3);
	}
	.choices {
		display: flex;
		gap: var(--space-2);
	}
</style>
