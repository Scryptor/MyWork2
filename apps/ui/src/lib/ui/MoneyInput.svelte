<script lang="ts">
	import { untrack } from 'svelte';
	import { formatMoney, parseMoney } from '#lib/format/money.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';

	let {
		value = $bindable(null),
		invalid = $bindable(false),
		nullable = true,
		currency = '₽',
		placeholder,
		label,
		hint,
		error,
		required = false,
		disabled = false,
		id
	}: {
		value: number | null;
		/** Out: the text is not an amount (the value keeps the last amount typed); forms do not submit. */
		invalid?: boolean;
		nullable?: boolean;
		currency?: string;
		placeholder?: string;
		label?: string;
		hint?: string;
		error?: string;
		required?: boolean;
		disabled?: boolean;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let focused = $state(false);
	/** The user typed in the field since it got focus: the text is theirs, not a copy of the value. */
	let dirty = false;
	/** What the field shows while focused or wrong: the typed text, or the value as plain digits. */
	let text = $state('');
	/** The error shows once the field is left or Enter is pressed and goes as soon as the text is an amount. */
	let showError = $state(false);
	/** The value the text stands for; any other value came from outside (another record, an undo). */
	let synced: number | null | undefined;

	function plain(v: number | null): string {
		return v === null ? '' : formatMoney(v, { currency: null }).replace(/\s/g, '');
	}

	/** Empty is null (0 when not nullable); undefined when the text is not an amount. */
	function amountOf(t: string): number | null | undefined {
		if (t.trim() === '') return nullable ? null : 0;
		return parseMoney(t) ?? undefined;
	}

	// A value from outside replaces the text, unless the field has focus: the user's own text stays until
	// they leave it, then the field shows the value (see `leave`).
	$effect.pre(() => {
		const v = value;
		untrack(() => {
			if (v === synced || (focused && dirty)) return;
			synced = v;
			text = plain(v);
			showError = false;
			if (invalid) invalid = false;
		});
	});

	const shown = $derived(
		focused || showError ? text : value === null ? '' : formatMoney(value, { currency: null })
	);

	/** The value changes only here, as the user types: an amount is written at once, anything else is not. */
	function input(next: string) {
		text = next;
		dirty = true;
		const amount = amountOf(next);
		const bad = amount === undefined;
		if (bad !== invalid) invalid = bad;
		if (amount === undefined) return;
		showError = false;
		synced = amount;
		if (amount !== value) value = amount;
	}

	function leave() {
		focused = false;
		const own = dirty;
		dirty = false;
		if (own && value !== synced) {
			// the value changed from outside while the user was typing
			synced = value;
			text = plain(value);
			showError = false;
			if (invalid) invalid = false;
			return;
		}
		showError = invalid;
	}
</script>

<Field
	{label}
	{hint}
	error={error ?? (showError ? ru.fields.moneyError : undefined)}
	{required}
	id={fid}
>
	{#snippet children(f)}
		<div class="mw-box" class:invalid={f.invalid} class:disabled>
			<input
				class="mw-input num"
				id={f.id}
				inputmode="decimal"
				autocomplete="off"
				{placeholder}
				{disabled}
				value={shown}
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
				onfocus={() => {
					if (!showError) text = plain(value);
					focused = true;
					dirty = false;
				}}
				oninput={(e) => input(e.currentTarget.value)}
				onblur={leave}
				onkeydown={(e) => {
					if (e.key === 'Enter') showError = invalid;
				}}
			/>
			<span class="mw-affix">{currency}</span>
		</div>
	{/snippet}
</Field>
