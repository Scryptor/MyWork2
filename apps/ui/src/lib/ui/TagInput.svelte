<!-- A list of short values typed one by one (the masters of the first run): Enter makes the text a chip, ×
     takes a chip away, Backspace in an empty field the last one. A repeat (any case) is not added twice.
     Leaving the field keeps what was typed. Enter in an empty field sends the form as usual. -->
<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { normalizeSearch } from '#lib/format/search.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';

	let {
		values = $bindable([]),
		label,
		hint,
		help,
		error,
		placeholder,
		disabled = false,
		removeLabel = ru.fields.removeTag
	}: {
		values: string[];
		label: string;
		hint?: string;
		/** «Что это?»: an icon right of the label with an explanation. */
		help?: string | readonly string[];
		error?: string;
		placeholder?: string;
		disabled?: boolean;
		/** The name of the × of a chip. */
		removeLabel?: (value: string) => string;
	} = $props();

	const uid = $props.id();
	let draft = $state('');

	/** Adds the typed text; false when there was nothing to add. */
	function commit(): boolean {
		const text = draft.trim().replace(/\s+/g, ' ');
		draft = '';
		if (text === '') return false;
		const key = normalizeSearch(text);
		if (!values.some((v) => normalizeSearch(v) === key)) values = [...values, text];
		return true;
	}

	function remove(value: string) {
		values = values.filter((v) => v !== value);
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && draft.trim() !== '') {
			e.preventDefault();
			commit();
		} else if (e.key === 'Backspace' && draft === '' && values.length > 0) {
			e.preventDefault();
			values = values.slice(0, -1);
		}
	}
</script>

<div class="tags">
	<Field {label} {hint} {help} {error} id={uid}>
		{#snippet children(f)}
			<div class="mw-box" class:invalid={f.invalid} class:disabled>
				<input
					class="mw-input"
					id={f.id}
					type="text"
					bind:value={draft}
					{placeholder}
					{disabled}
					aria-describedby={f.describedBy}
					aria-invalid={f.invalid ? 'true' : undefined}
					{onkeydown}
					onblur={commit}
				/>
			</div>
		{/snippet}
	</Field>
	{#if values.length > 0}
		<ul class="chips" aria-label={label}>
			{#each values as value (value)}
				<li class="chip">
					<span class="text">{value}</span>
					<button
						type="button"
						class="remove"
						aria-label={removeLabel(value)}
						{disabled}
						onclick={() => remove(value)}
					>
						<X size={14} aria-hidden="true" />
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.tags {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1-5);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		box-sizing: border-box;
		min-height: var(--control-sm);
		max-width: 100%;
		padding: 0 var(--space-1) 0 var(--space-3);
		border: 1px solid var(--accent);
		border-radius: var(--radius-full);
		background: var(--accent-soft);
		color: var(--accent-text);
		font-size: var(--text-md);
	}
	.text {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.remove {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
		width: var(--control-sm);
		height: var(--control-sm);
		padding: 0;
		border: 0;
		border-radius: var(--radius-full);
		background: transparent;
		color: inherit;
		cursor: pointer;
	}
	.remove:hover:not(:disabled) {
		background: var(--surface-3);
	}
</style>
