<script lang="ts">
	import { formatMoney, parseMoney } from '#lib/format/money.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';

	let {
		value = $bindable(null),
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
	let text = $state('');
	let parseError = $state(false);

	function plain(v: number | null): string {
		return v === null ? '' : formatMoney(v, { currency: null }).replace(/\s/g, '');
	}
	const shown = $derived(
		focused || parseError ? text : value === null ? '' : formatMoney(value, { currency: null })
	);

	function commit() {
		const t = text.trim();
		if (t === '') {
			value = nullable ? null : 0;
			parseError = false;
			return;
		}
		const parsed = parseMoney(t);
		if (parsed === null) {
			parseError = true;
			return;
		}
		value = parsed;
		parseError = false;
	}
</script>

<Field
	{label}
	{hint}
	error={error ?? (parseError ? ru.fields.moneyError : undefined)}
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
					if (!parseError) text = plain(value);
					focused = true;
				}}
				oninput={(e) => (text = e.currentTarget.value)}
				onblur={() => {
					commit();
					focused = false;
				}}
				onkeydown={(e) => {
					if (e.key === 'Enter') commit();
				}}
			/>
			<span class="mw-affix">{currency}</span>
		</div>
	{/snippet}
</Field>
