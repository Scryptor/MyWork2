<!-- A number of a template (millimetres, points) typed into a field: every value within the limits of the
     model (`Template::validate`) is an edit at once; anything else stays in the field with its mistake,
     shown once the field is left, and is not sent. `nullable`: an empty field is null. -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import { formatMm, parseMm } from '#lib/print/settings.svelte.js';
	import TextInput from '#lib/ui/TextInput.svelte';

	let {
		label,
		value,
		min,
		max,
		nullable = false,
		hint,
		onchange
	}: {
		label: string;
		value: number | null;
		min: number;
		max: number;
		nullable?: boolean;
		hint?: string;
		/** A value within the limits, as it is typed. */
		onchange: (n: number | null) => void;
	} = $props();

	const t = ru.templates;
	/** What is typed while the field is in use; null: the field shows the value. */
	let draft = $state<string | null>(null);
	let error = $state<string | undefined>();

	function mistake(text: string): string | undefined {
		if (nullable && text.trim() === '') return undefined;
		const n = parseMm(text);
		if (n === null) return t.notNumber;
		return n >= min && n <= max ? undefined : t.range(min, max);
	}

	function input(text: string) {
		draft = text;
		const wrong = mistake(text);
		if (wrong === undefined) {
			error = undefined;
			onchange(text.trim() === '' ? null : parseMm(text));
		} else if (error !== undefined) error = wrong;
	}

	function leave() {
		if (draft === null) return;
		error = mistake(draft);
		if (error === undefined) draft = null;
	}
</script>

<TextInput
	{label}
	{hint}
	inputmode="decimal"
	autocomplete="off"
	{error}
	bind:value={() => draft ?? (value === null ? '' : formatMm(value)), input}
	onblur={leave}
/>
