<!-- Several values from a list. The chosen ones are tags in the field: × takes one away, Backspace in an
     empty field the last one. Typing narrows the list (any case, ё as е), ↓/↑ and Enter or a click pick;
     the chosen values are not offered again and the list stays open for the next one. With `allowCustom`
     the text typed is offered at the end of the list as «Добавить «…»»: Enter or a click adds it and
     tells `oncreate`. The list draws at most `limit` rows, the rest is found by typing. Text typed and not
     chosen is never added by itself (it may be a piece of a search): the owner sees it as `pending`. -->
<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { Combobox } from 'bits-ui';
	import { untrack } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import { normalizeSearch } from '#lib/format/search.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';
	import './popup.css';

	let {
		selected = $bindable([]),
		pending = $bindable(''),
		options,
		label,
		help,
		allowCustom = false,
		oncreate,
		placeholder,
		id,
		disabled = false,
		limit = 50,
		error
	}: {
		selected: string[];
		/** The text in the field, not chosen yet: Enter adds it, a pick or clearing the field empties it. */
		pending?: string;
		/** In the order to offer them (the most used first). */
		options: string[];
		label: string;
		/** «Что это?»: an icon right of the label with an explanation. */
		help?: string | readonly string[];
		allowCustom?: boolean;
		/** A value typed that is not among the options was added. */
		oncreate?: (value: string) => void;
		/** Shown while nothing is chosen. */
		placeholder?: string;
		id?: string;
		disabled?: boolean;
		/** Rows of the list at most. */
		limit?: number;
		error?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let open = $state(false);
	let box = $state<HTMLElement | null>(null);
	let input = $state<HTMLInputElement | null>(null);
	let list = $state<HTMLElement | null>(null);

	$effect(() => {
		if (open) return untrack(() => layers.open('popup', () => (open = false)));
	});

	/** A value repeated in `selected` (an old draft) is one tag; the next change drops the repeat. */
	const unique = $derived([...new Set(selected)]);
	const query = $derived(normalizeSearch(pending));
	const chosen = $derived(new Set(selected.map(normalizeSearch)));
	const matches = $derived.by(() => {
		const found = options.filter((o) => {
			const key = normalizeSearch(o);
			return !chosen.has(key) && key.includes(query);
		});
		// The very text typed comes first, whatever its place in the list.
		const exact = found.findIndex((o) => normalizeSearch(o) === query);
		if (exact > 0) found.unshift(...found.splice(exact, 1));
		return found.slice(0, limit);
	});
	/** The text typed as a value of its own: neither an option nor chosen already. */
	const custom = $derived(
		allowCustom &&
			query !== '' &&
			!chosen.has(query) &&
			!options.some((o) => normalizeSearch(o) === query)
			? pending.trim().replace(/\s+/g, ' ')
			: null
	);

	/** bits-ui only adds (the chosen values are not in the list); the tags take them away. */
	function onValueChange(next: string[]) {
		// Read before `selected` changes: the owner's options may take in the chosen values at once.
		const created = next.filter((v) => !unique.includes(v) && !options.includes(v));
		selected = next;
		pending = '';
		for (const v of created) oncreate?.(v);
	}

	function remove(value: string) {
		selected = selected.filter((v) => v !== value);
	}

	function onInput(e: Event & { currentTarget: HTMLInputElement }) {
		pending = e.currentTarget.value;
		open = true;
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Backspace' && pending === '' && unique.length > 0) {
			e.preventDefault();
			remove(unique[unique.length - 1]!);
			return;
		}
		if (e.key !== 'Enter' || e.isComposing) return;
		// Ctrl/⌘+Enter accepts the form (its hotkey listens on the window, whatever is prevented here):
		// bits-ui would pick the highlighted row on the way. The text typed stays pending for the owner.
		if (e.ctrlKey || e.metaKey) {
			e.preventDefault();
			return;
		}
		// A highlighted row is bits-ui's to pick.
		if (open && list?.querySelector('[data-highlighted]')) return;
		// Chosen already: the text has nothing left to add, it would only stay pending.
		if (query !== '' && chosen.has(query)) {
			e.preventDefault();
			pending = '';
			return;
		}
		const value = custom ?? (query !== '' ? matches[0] : undefined);
		if (value === undefined) return;
		e.preventDefault();
		onValueChange([...unique, value]);
	}

	function openList() {
		if (!disabled) open = true;
	}
</script>

<Field {label} {help} {error} id={fid}>
	{#snippet children(f)}
		<Combobox.Root
			type="multiple"
			{disabled}
			bind:open
			value={unique}
			{onValueChange}
			inputValue={pending}
		>
			<!-- A press beside the tags is a press in the field. -->
			<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
			<div
				class="mw-box multi"
				class:invalid={f.invalid}
				class:disabled
				bind:this={box}
				onclick={(e) => {
					if (e.target !== e.currentTarget) return;
					input?.focus();
					openList();
				}}
			>
				{#each unique as value (value)}
					<span class="tag">
						<span class="text">{value}</span>
						<button
							type="button"
							class="remove"
							aria-label={ru.fields.removeChoice(value)}
							{disabled}
							onclick={() => remove(value)}
						>
							<X size={14} aria-hidden="true" />
						</button>
					</span>
				{/each}
				<Combobox.Input
					id={f.id}
					class="mw-input"
					bind:ref={input}
					placeholder={unique.length === 0 ? placeholder : undefined}
					autocomplete="off"
					aria-describedby={f.describedBy}
					aria-invalid={f.invalid ? 'true' : undefined}
					oninput={onInput}
					onkeydown={onKeydown}
					onclick={openList}
				>
					{#snippet child({ props })}
						<!-- The field holds what is typed: bits-ui would put the label of a pick there. -->
						<input {...props} value={pending} />
					{/snippet}
				</Combobox.Input>
			</div>
			<Combobox.Portal>
				<Combobox.Content class="mw-popup" sideOffset={4} customAnchor={box} bind:ref={list}>
					{#each matches as o (o)}
						<Combobox.Item class="mw-option" value={o} label={o}>{o}</Combobox.Item>
					{/each}
					{#if custom !== null}
						<Combobox.Item class="mw-option add" value={custom} label={custom}>
							{ru.fields.addChoice(custom)}
						</Combobox.Item>
					{:else if matches.length === 0}
						<div class="mw-empty">{ru.common.nothingFound}</div>
					{/if}
				</Combobox.Content>
			</Combobox.Portal>
		</Combobox.Root>
	{/snippet}
</Field>

<style>
	/* Tags and the input share the lines of the box: a row is as tall as a plain field. */
	.multi {
		flex-wrap: wrap;
		gap: var(--space-1);
		height: auto;
		min-height: var(--control-md);
		padding: calc((var(--control-md) - var(--control-sm)) / 2 - 1px) var(--space-1);
		cursor: text;
	}
	.multi :global(.mw-input) {
		flex: 1 0 6rem;
		height: var(--control-sm);
		padding: 0 var(--space-1);
	}
	.multi.disabled {
		cursor: not-allowed;
	}
	.tag {
		display: inline-flex;
		align-items: center;
		gap: var(--space-0-5);
		box-sizing: border-box;
		min-height: var(--control-sm);
		max-width: 100%;
		padding: 0 0 0 var(--space-2);
		border: 1px solid var(--accent);
		border-radius: var(--radius-sm);
		background: var(--accent-soft);
		color: var(--accent-text);
		cursor: default;
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
		border-radius: var(--radius-sm);
		background: transparent;
		color: inherit;
		cursor: pointer;
	}
	.remove:hover:not(:disabled) {
		background: var(--surface-3);
	}
	.remove:disabled {
		cursor: not-allowed;
	}
	:global(.mw-option.add) {
		color: var(--accent-text);
	}
</style>
