<!-- A whole number with «−» and «＋»: a quantity of things. Digits only; a number out of `min…max` is
     brought back when the field is left. -->
<script lang="ts">
	import Minus from '@lucide/svelte/icons/minus';
	import Plus from '@lucide/svelte/icons/plus';
	import { untrack } from 'svelte';
	import { groupDigits } from '#lib/format/digits.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';

	let {
		value = $bindable(0),
		min = 0,
		max = 1_000_000,
		label,
		hint,
		help,
		error,
		required = false,
		disabled = false,
		id,
		onchange
	}: {
		value: number;
		min?: number;
		max?: number;
		label?: string;
		hint?: string;
		/** «Что это?»: an icon right of the label with an explanation. */
		help?: string | readonly string[];
		error?: string;
		required?: boolean;
		disabled?: boolean;
		id?: string;
		/** The user changed the value (a button, typing); not called for a value that came from outside. */
		onchange?: (v: number) => void;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let focused = $state(false);
	/** What the field shows while it has focus: the digits typed. */
	let text = $state('');
	let synced: number | undefined;

	const clamp = (n: number) => Math.min(max, Math.max(min, n));

	// A value from outside replaces the text, unless the field has focus.
	$effect.pre(() => {
		const v = value;
		untrack(() => {
			if (v === synced) return;
			synced = v;
			if (!focused) text = String(v);
		});
	});

	function set(next: number) {
		if (next === value) return;
		synced = next;
		value = next;
		onchange?.(next);
	}

	function input(el: HTMLInputElement) {
		const digits = el.value.replace(/\D/g, '');
		// A letter typed leaves the text as it was: the field itself has to be told.
		el.value = digits;
		text = digits;
		if (digits === '') return;
		set(clamp(Number(digits)));
	}

	function leave() {
		focused = false;
		// Empty goes back to the minimum; a number out of range was already brought in by `input`.
		if (text === '') set(min);
		text = String(value);
	}
</script>

<Field {label} {hint} {help} {error} {required} id={fid}>
	{#snippet children(f)}
		<div class="mw-box qty" class:invalid={f.invalid} class:disabled>
			<button
				type="button"
				class="step"
				aria-label={ru.fields.quantityLess}
				disabled={disabled || value <= min}
				onclick={() => set(clamp(value - 1))}
			>
				<Minus size={14} aria-hidden="true" />
			</button>
			<input
				class="mw-input num"
				id={f.id}
				inputmode="numeric"
				autocomplete="off"
				{disabled}
				value={focused ? text : groupDigits(String(value))}
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
				onfocus={() => {
					text = String(value);
					focused = true;
				}}
				oninput={(e) => input(e.currentTarget)}
				onblur={leave}
			/>
			<button
				type="button"
				class="step"
				aria-label={ru.fields.quantityMore}
				disabled={disabled || value >= max}
				onclick={() => set(clamp(value + 1))}
			>
				<Plus size={14} aria-hidden="true" />
			</button>
		</div>
	{/snippet}
</Field>

<style>
	.qty {
		padding: 0 var(--space-1);
	}
	.qty .mw-input {
		text-align: center;
	}
	.step {
		display: inline-flex;
		flex: none;
		align-items: center;
		justify-content: center;
		width: var(--control-sm);
		height: var(--control-sm);
		padding: 0;
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text-2);
		cursor: pointer;
	}
	.step:hover:not(:disabled) {
		background: var(--surface-2);
		color: var(--text);
	}
	.step:focus-visible {
		outline: 2px solid var(--focus);
	}
	.step:disabled {
		color: var(--text-disabled);
		cursor: not-allowed;
	}
</style>
