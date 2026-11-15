<script lang="ts">
	import { formatPhone, phoneDigits } from '#lib/format/phone.js';
	import Field from './Field.svelte';

	let {
		value = $bindable(''),
		placeholder,
		label,
		hint,
		error,
		required = false,
		disabled = false,
		id
	}: {
		value: string;
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
	const shown = $derived(focused ? text : formatPhone(value));
</script>

<Field {label} {hint} {error} {required} id={fid}>
	{#snippet children(f)}
		<div class="mw-box" class:invalid={f.invalid} class:disabled>
			<input
				class="mw-input num"
				id={f.id}
				type="tel"
				inputmode="tel"
				autocomplete="tel"
				{placeholder}
				{disabled}
				value={shown}
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
				onfocus={() => {
					text = formatPhone(value);
					focused = true;
				}}
				oninput={(e) => {
					text = e.currentTarget.value;
					value = phoneDigits(text);
				}}
				onblur={() => (focused = false)}
			/>
		</div>
	{/snippet}
</Field>
