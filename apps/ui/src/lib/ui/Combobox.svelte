<script lang="ts">
	import { Combobox } from 'bits-ui';
	import { untrack } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import { normalizeSearch } from '#lib/format/search.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';
	import './popup.css';

	const MAX_ITEMS = 50;

	let {
		value = $bindable(''),
		items,
		allowCustom = true,
		placeholder,
		emptyText = ru.common.nothingFound,
		filter = true,
		onselect,
		onenter,
		onblur,
		label,
		hint,
		error,
		required = false,
		disabled = false,
		id
	}: {
		value: string;
		items: { value: string; hint?: string }[];
		allowCustom?: boolean;
		placeholder?: string;
		emptyText?: string;
		/** false: the items are shown as given (the caller already filtered them, e.g. the server). */
		filter?: boolean;
		onselect?: (v: string) => void;
		/** Enter in the field with nothing to pick: the text typed is final. */
		onenter?: () => void;
		/** The field is left. */
		onblur?: () => void;
		label?: string;
		hint?: string;
		error?: string;
		required?: boolean;
		disabled?: boolean;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let open = $state(false);
	let text = $derived(value);

	$effect(() => {
		if (open) return untrack(() => layers.open('popup', () => (open = false)));
	});

	const filtered = $derived.by(() => {
		if (!filter) return items.slice(0, MAX_ITEMS);
		const q = normalizeSearch(text);
		return items
			.filter((i) => q === '' || normalizeSearch(i.value).includes(q))
			.slice(0, MAX_ITEMS);
	});

	function pick(v: string) {
		value = v;
		text = v;
		open = false;
		onselect?.(v);
	}

	function onInput(e: Event & { currentTarget: HTMLInputElement }) {
		text = e.currentTarget.value;
		open = true;
		if (allowCustom) value = text;
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key !== 'Enter') return;
		if (!open) {
			onenter?.();
			return;
		}
		const highlighted = document.querySelector('[data-combobox-item][data-highlighted]');
		if (highlighted) return;
		const first = filtered[0];
		if (first && normalizeSearch(text) !== '') {
			e.preventDefault();
			pick(first.value);
		} else onenter?.();
	}

	function onBlur() {
		if (!allowCustom) text = value;
		onblur?.();
	}
</script>

<Field {label} {hint} {error} {required} id={fid}>
	{#snippet children(f)}
		<Combobox.Root
			type="single"
			{disabled}
			bind:open
			{value}
			onValueChange={(v) => {
				if (v) pick(v);
			}}
			inputValue={text}
		>
			<div class="mw-box" class:invalid={f.invalid} class:disabled>
				<Combobox.Input
					id={f.id}
					class="mw-input"
					{placeholder}
					autocomplete="off"
					aria-describedby={f.describedBy}
					aria-invalid={f.invalid ? 'true' : undefined}
					aria-required={required ? 'true' : undefined}
					oninput={onInput}
					onkeydown={onKeydown}
					onblur={onBlur}
				/>
			</div>
			<Combobox.Portal>
				<Combobox.Content class="mw-popup" sideOffset={4}>
					{#each filtered as i (i.value)}
						<Combobox.Item class="mw-option" value={i.value} label={i.value}>
							<span>{i.value}</span>
							{#if i.hint}<span class="mw-hint">{i.hint}</span>{/if}
						</Combobox.Item>
					{:else}
						<div class="mw-empty">{emptyText}</div>
					{/each}
				</Combobox.Content>
			</Combobox.Portal>
		</Combobox.Root>
	{/snippet}
</Field>
