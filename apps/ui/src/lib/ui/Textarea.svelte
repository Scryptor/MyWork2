<script lang="ts">
	import Field from './Field.svelte';

	let {
		value = $bindable(''),
		minRows = 2,
		maxRows = 12,
		placeholder,
		label,
		hint,
		help,
		error,
		required = false,
		disabled = false,
		onblur,
		id
	}: {
		value: string;
		minRows?: number;
		maxRows?: number;
		placeholder?: string;
		label?: string;
		hint?: string;
		/** «Что это?»: an icon right of the label with an explanation. */
		help?: string | readonly string[];
		error?: string;
		required?: boolean;
		disabled?: boolean;
		onblur?: () => void;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let el = $state<HTMLTextAreaElement | null>(null);

	function resize() {
		if (!el) return;
		const cs = getComputedStyle(el);
		const line = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.4;
		el.style.height = 'auto';
		const min = line * minRows;
		const max = line * maxRows;
		el.style.height = `${Math.min(Math.max(el.scrollHeight, min), max)}px`;
		el.style.overflowY = el.scrollHeight > max ? 'auto' : 'hidden';
	}

	$effect(() => {
		void value;
		resize();
	});
</script>

<Field {label} {hint} {help} {error} {required} id={fid}>
	{#snippet children(f)}
		<div class="mw-box multiline" class:invalid={f.invalid} class:disabled>
			<textarea
				class="mw-input"
				id={f.id}
				bind:this={el}
				bind:value
				rows={minRows}
				{placeholder}
				{disabled}
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
				{onblur}></textarea>
		</div>
	{/snippet}
</Field>

<style>
	textarea {
		resize: none;
		height: auto;
		line-height: var(--leading-normal);
	}
</style>
