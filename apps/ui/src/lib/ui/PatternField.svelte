<!-- The unlock pattern as a field (plan 8.2): the button says whether a pattern is set, the grid opens in
     a popover, and «Готово» takes what was pressed there. Closed any other way, the popover leaves the
     pattern as it was. -->
<script lang="ts">
	import Grip from '@lucide/svelte/icons/grip';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';
	import { patternReady } from '#lib/orders/pattern.js';
	import Button from './Button.svelte';
	import Field from './Field.svelte';
	import PatternInput from './PatternInput.svelte';
	import Popover from './Popover.svelte';

	let {
		value,
		onchange,
		label,
		help,
		disabled = false,
		id
	}: {
		/** The points in their order: «1596»; empty — no pattern. */
		value: string;
		/** «Готово» with a pattern other than `value`: empty or of four points and more. */
		onchange: (value: string) => void;
		label: string;
		/** «Что это?»: an icon right of the label with an explanation. */
		help?: string | readonly string[];
		disabled?: boolean;
		id?: string;
	} = $props();

	const t = ru.pattern;
	const uid = $props.id();
	const fid = $derived(id ?? uid);
	const stateId = $derived(`${fid}-state`);

	let open = $state(false);
	/** What is pressed in the open popover; «Готово» makes it the value. */
	let draft = $state('');
	/** Every opening starts from the value: what was pressed and not taken is gone. */
	function setOpen(next: boolean) {
		if (next && !open) draft = value;
		open = next;
	}

	const canTake = $derived(draft === '' || patternReady(draft));
	function done() {
		if (!canTake) return;
		open = false;
		if (draft !== value) onchange(draft);
	}
	// While the popover is open, Ctrl/⌘+Enter is «Готово», not the form underneath.
	$effect(() => {
		if (open) return submitShortcut(done);
	});

	const text = $derived(value === '' ? t.notSet : t.set(countLabel(value.length, t.pointForms)));
</script>

<Field {label} {help} id={fid}>
	{#snippet children(f)}
		<Popover bind:open={() => open, setOpen} {label}>
			{#snippet trigger(props)}
				<button
					{...props}
					id={f.id}
					type="button"
					class="mw-box select trigger"
					class:disabled
					{disabled}
					aria-describedby={[f.describedBy, stateId].filter(Boolean).join(' ')}
				>
					<span id={stateId} class="val" class:unset={value === ''}>{text}</span>
					<Grip size={16} aria-hidden="true" class="grip" />
				</button>
			{/snippet}
			<div class="body">
				<PatternInput bind:value={draft} />
				<div class="done">
					<Button variant="primary" disabled={!canTake} onclick={done}>{t.done}</Button>
				</div>
			</div>
		</Popover>
	{/snippet}
</Field>

<style>
	.trigger {
		justify-content: space-between;
		text-align: left;
		cursor: pointer;
	}
	.trigger:disabled {
		cursor: not-allowed;
	}
	.val {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.unset {
		color: var(--text-3);
	}
	.trigger:disabled .val {
		color: var(--text-disabled);
	}
	.trigger :global(.grip) {
		flex: none;
		color: var(--text-3);
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.done {
		display: flex;
		justify-content: flex-end;
	}
</style>
