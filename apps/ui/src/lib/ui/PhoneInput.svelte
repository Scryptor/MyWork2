<script lang="ts">
	import { untrack } from 'svelte';
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
	/** The user typed in the field since it got focus: the text is theirs, not a copy of the value. */
	let dirty = false;
	let text = $state('');
	/** The digits the text stands for; any other value came from outside (another record). */
	let synced: string | undefined;

	// A number from outside replaces the text, unless the field has focus: the user's own text stays until
	// they leave it, then the field shows the value.
	$effect.pre(() => {
		const v = value;
		untrack(() => {
			if (v === synced || (focused && dirty)) return;
			synced = v;
			text = formatPhone(v);
		});
	});

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
					dirty = true;
					const digits = phoneDigits(text);
					synced = digits;
					if (digits !== value) value = digits;
				}}
				onblur={() => {
					focused = false;
					dirty = false;
					synced = value;
				}}
			/>
		</div>
	{/snippet}
</Field>
